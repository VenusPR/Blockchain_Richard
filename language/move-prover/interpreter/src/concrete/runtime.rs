// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use bytecode::{function_target::FunctionTarget, function_target_pipeline::FunctionTargetsHolder};
use move_binary_format::errors::{Location, PartialVMError, PartialVMResult, VMResult};
use move_core_types::{
    language_storage::{StructTag, TypeTag},
    value::MoveValue,
    vm_status::StatusCode,
};
use move_model::{
    model::{AbilitySet, FunctionEnv, GlobalEnv, TypeParameter},
    ty as MT,
};

use crate::{
    concrete::{
        player,
        ty::{
            convert_model_base_type, BaseType, IntType, PrimitiveType, StructField,
            StructInstantiation,
        },
        value::{GlobalState, TypedValue},
    },
    shared::{ident::StructIdent, variant::choose_variant},
};

/// A stackless bytecode runtime in charge of pre- and post-execution checking, conversion, and
/// monitoring. The main, step-by-step interpretation loop is delegated to the `Player` instance.
pub struct Runtime<'env> {
    env: &'env GlobalEnv,
    functions: &'env FunctionTargetsHolder,
    verbose: bool,
}

impl<'env> Runtime<'env> {
    //
    // public interfaces
    //

    /// Construct a runtime with all information pre-loaded.
    pub fn new(
        env: &'env GlobalEnv,
        functions: &'env FunctionTargetsHolder,
        verbose: bool,
    ) -> Self {
        Self {
            env,
            functions,
            verbose,
        }
    }

    /// Execute a function (identified by `fun_id`) with given type arguments, arguments, and a
    /// snapshot of the global state. Returns a tuple contains both the result of the execution
    /// and the new global state.
    ///
    /// NOTE: there may be multiple variants of the same function registered in the
    /// FunctionTargetsHolder, all variants will be executed and this function only return a new
    /// global state if results from all variants converges. Otherwise, the old state is returned
    /// and the VMResult is set to an error accordingly.
    pub fn execute(
        &self,
        fun_env: &FunctionEnv,
        ty_args: &[TypeTag],
        args: &[MoveValue],
        mut global_state: GlobalState,
    ) -> (VMResult<Vec<TypedValue>>, GlobalState) {
        // check and convert type arguments
        match check_type_instantiation(self.env, &fun_env.get_type_parameters(), ty_args) {
            Ok(_) => (),
            Err(err) => {
                return (Err(err.finish(Location::Undefined)), global_state);
            }
        };
        let mut converted_ty_args = vec![];
        for ty_arg in ty_args {
            match convert_move_type_tag(self.env, ty_arg) {
                Ok(converted) => converted_ty_args.push(converted),
                Err(err) => {
                    return (Err(err.finish(Location::Undefined)), global_state);
                }
            }
        }

        // check and convert value arguments
        let params = fun_env.get_parameters();
        if params.len() != args.len() {
            return (
                Err(
                    PartialVMError::new(StatusCode::NUMBER_OF_ARGUMENTS_MISMATCH)
                        .finish(Location::Undefined),
                ),
                global_state,
            );
        }
        let mut converted_args = vec![];
        for (i, arg) in args.iter().enumerate() {
            let local_ty = fun_env.get_local_type(i);
            if cfg!(debug_assertions) {
                let param_ty = &params.get(i).unwrap().1;
                assert_eq!(&local_ty, param_ty);
            }
            // NOTE: for historical reasons, we may receive `&signer` as arguments
            // TODO (mengxu): clean this up when we no longer accept `&signer` as valid arguments
            // for transaction scripts and `public(script)` functions.
            match local_ty {
                MT::Type::Reference(false, base_ty)
                    if matches!(*base_ty, MT::Type::Primitive(MT::PrimitiveType::Signer)) =>
                {
                    match arg {
                        MoveValue::Address(v) => {
                            converted_args.push(TypedValue::mk_signer(*v));
                        }
                        _ => {
                            return (
                                Err(PartialVMError::new(StatusCode::TYPE_MISMATCH)
                                    .finish(Location::Undefined)),
                                global_state,
                            );
                        }
                    }
                }
                _ => {
                    let base_ty = convert_model_base_type(self.env, &local_ty, &converted_ty_args);
                    match convert_move_value(self.env, arg, &base_ty) {
                        Ok(converted) => {
                            converted_args.push(converted);
                        }
                        Err(err) => {
                            return (Err(err.finish(Location::Undefined)), global_state);
                        }
                    }
                }
            }
        }

        let fun_target = choose_variant(self.functions, &fun_env);
        let result = self.execute_target(
            fun_target,
            &converted_ty_args,
            &converted_args,
            &mut global_state,
        );
        (result, global_state)
    }

    //
    // execution internals
    //

    fn execute_target(
        &self,
        fun_target: FunctionTarget,
        ty_args: &[BaseType],
        args: &[TypedValue],
        global_state: &mut GlobalState,
    ) -> VMResult<Vec<TypedValue>> {
        let module_id = fun_target.module_env().get_verified_module().self_id();
        player::entrypoint(
            self.functions,
            fun_target,
            ty_args,
            args.to_vec(),
            global_state,
            self.verbose,
        )
        .map_err(|err| err.finish(Location::Module(module_id)))
    }
}

//**************************************************************************************************
// Utilities
//**************************************************************************************************

pub fn convert_move_type_tag(env: &GlobalEnv, tag: &TypeTag) -> PartialVMResult<BaseType> {
    let converted = match tag {
        TypeTag::Bool => BaseType::mk_bool(),
        TypeTag::U8 => BaseType::mk_u8(),
        TypeTag::U64 => BaseType::mk_u64(),
        TypeTag::U128 => BaseType::mk_u128(),
        TypeTag::Address => BaseType::mk_address(),
        TypeTag::Signer => BaseType::mk_signer(),
        TypeTag::Vector(elem_tag) => BaseType::mk_vector(convert_move_type_tag(env, elem_tag)?),
        TypeTag::Struct(struct_tag) => {
            BaseType::mk_struct(convert_move_struct_tag(env, struct_tag)?)
        }
    };
    Ok(converted)
}

pub fn convert_move_struct_tag(
    env: &GlobalEnv,
    struct_tag: &StructTag,
) -> PartialVMResult<StructInstantiation> {
    // get struct env
    let struct_id = env.find_struct_by_tag(struct_tag).ok_or_else(|| {
        PartialVMError::new(StatusCode::TYPE_RESOLUTION_FAILURE).with_message(format!(
            "Cannot find struct `{}::{}::{}`",
            struct_tag.address.short_str_lossless(),
            struct_tag.module,
            struct_tag.name,
        ))
    })?;
    let struct_env = env.get_struct(struct_id);
    let ident = StructIdent::new(&struct_env);

    // check and convert type args
    check_type_instantiation(
        env,
        &struct_env.get_type_parameters(),
        &struct_tag.type_params,
    )?;
    let insts = struct_tag
        .type_params
        .iter()
        .map(|ty_arg| convert_move_type_tag(env, ty_arg))
        .collect::<PartialVMResult<Vec<_>>>()?;

    // collect fields
    let fields = struct_env
        .get_fields()
        .map(|field_env| {
            let field_name = env.symbol_pool().string(field_env.get_name()).to_string();
            let field_ty = convert_model_base_type(env, &field_env.get_type(), &insts);
            StructField {
                name: field_name,
                ty: field_ty,
            }
        })
        .collect();

    Ok(StructInstantiation {
        ident,
        insts,
        fields,
    })
}

pub fn convert_move_value(
    env: &GlobalEnv,
    val: &MoveValue,
    ty: &BaseType,
) -> PartialVMResult<TypedValue> {
    let converted = match (val, ty) {
        (MoveValue::Bool(v), BaseType::Primitive(PrimitiveType::Bool)) => TypedValue::mk_bool(*v),
        (MoveValue::U8(v), BaseType::Primitive(PrimitiveType::Int(IntType::U8))) => {
            TypedValue::mk_u8(*v)
        }
        (MoveValue::U64(v), BaseType::Primitive(PrimitiveType::Int(IntType::U64))) => {
            TypedValue::mk_u64(*v)
        }
        (MoveValue::U128(v), BaseType::Primitive(PrimitiveType::Int(IntType::U128))) => {
            TypedValue::mk_u128(*v)
        }
        (MoveValue::Address(v), BaseType::Primitive(PrimitiveType::Address)) => {
            TypedValue::mk_address(*v)
        }
        (MoveValue::Signer(v), BaseType::Primitive(PrimitiveType::Signer)) => {
            TypedValue::mk_signer(*v)
        }
        (MoveValue::Vector(v), BaseType::Vector(elem)) => {
            let converted = v
                .iter()
                .map(|e| convert_move_value(env, e, elem))
                .collect::<PartialVMResult<Vec<_>>>()?;
            TypedValue::mk_vector(*elem.clone(), converted)
        }
        (MoveValue::Struct(v), BaseType::Struct(inst)) => {
            let fields = v.fields();
            if fields.len() != inst.fields.len() {
                return Err(PartialVMError::new(StatusCode::TYPE_MISMATCH));
            }
            let converted = fields
                .iter()
                .zip(inst.fields.iter())
                .map(|(f, info)| convert_move_value(env, f, &info.ty))
                .collect::<PartialVMResult<Vec<_>>>()?;
            TypedValue::mk_struct(inst.clone(), converted)
        }
        _ => {
            return Err(PartialVMError::new(StatusCode::TYPE_MISMATCH));
        }
    };
    Ok(converted)
}

fn check_type_instantiation(
    env: &GlobalEnv,
    params: &[TypeParameter],
    args: &[TypeTag],
) -> PartialVMResult<()> {
    if params.len() != args.len() {
        return Err(PartialVMError::new(
            StatusCode::NUMBER_OF_TYPE_ARGUMENTS_MISMATCH,
        ));
    }
    for (arg, param) in args.iter().zip(params) {
        if !param.1 .0.is_subset(get_abilities(env, arg)?) {
            return Err(PartialVMError::new(StatusCode::CONSTRAINT_NOT_SATISFIED));
        }
    }
    Ok(())
}

fn get_abilities(env: &GlobalEnv, ty: &TypeTag) -> PartialVMResult<AbilitySet> {
    let abilities = match ty {
        TypeTag::Bool | TypeTag::U8 | TypeTag::U64 | TypeTag::U128 | TypeTag::Address => {
            AbilitySet::PRIMITIVES
        }
        TypeTag::Signer => AbilitySet::SIGNER,
        TypeTag::Vector(elem_ty) => AbilitySet::polymorphic_abilities(
            AbilitySet::VECTOR,
            vec![get_abilities(env, elem_ty)?],
        ),
        TypeTag::Struct(struct_tag) => {
            let struct_id = env.find_struct_by_tag(struct_tag).ok_or_else(|| {
                PartialVMError::new(StatusCode::TYPE_RESOLUTION_FAILURE).with_message(format!(
                    "Cannot find struct `{}::{}::{}`",
                    struct_tag.address.short_str_lossless(),
                    struct_tag.module,
                    struct_tag.name,
                ))
            })?;
            let struct_env = env.get_struct(struct_id);
            let ty_arg_abilities = struct_tag
                .type_params
                .iter()
                .map(|arg| get_abilities(env, arg))
                .collect::<PartialVMResult<Vec<_>>>()?;
            AbilitySet::polymorphic_abilities(struct_env.get_abilities(), ty_arg_abilities)
        }
    };
    Ok(abilities)
}
