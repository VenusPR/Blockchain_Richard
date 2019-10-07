use super::dispatch::NativeReturnStatus;
use crate::value::Value;
use libra_types::{account_address::AccountAddress, byte_array::ByteArray};
use std::collections::VecDeque;

pub fn native_bytearray_concat(mut arguments: VecDeque<Value>) -> NativeReturnStatus {
    if arguments.len() != 2 {
        return NativeReturnStatus::InvalidArguments;
    }
    let arg2 = pop_arg!(arguments, ByteArray);
    let arg1 = pop_arg!(arguments, ByteArray);
    let mut return_val = arg1.as_bytes().to_vec();
    return_val.extend_from_slice(arg2.as_bytes());

    // TODO: Figure out the gas cost for concatenation.
    let cost = return_val.len() as u64;
    let return_values = vec![Value::byte_array(ByteArray::new(return_val))];
    NativeReturnStatus::Success {
        cost,
        return_values,
    }
}

pub fn native_address_to_bytes(mut arguments: VecDeque<Value>) -> NativeReturnStatus {
    if arguments.len() != 1 {
        return NativeReturnStatus::InvalidArguments;
    }
    let arg = pop_arg!(arguments, AccountAddress);
    let return_val = arg.to_vec();

    // TODO: Figure out the gas cost for conversion.
    let cost = return_val.len() as u64;
    let return_values = vec![Value::byte_array(ByteArray::new(return_val))];
    NativeReturnStatus::Success {
        cost,
        return_values,
    }
}

pub fn native_u64_to_bytes(mut arguments: VecDeque<Value>) -> NativeReturnStatus {
    if arguments.len() != 1 {
        return NativeReturnStatus::InvalidArguments;
    }
    let arg = pop_arg!(arguments, u64);
    let return_val: Vec<u8> = arg.to_le_bytes().to_vec();

    // TODO: Figure out the gas cost for conversion.
    let cost = return_val.len() as u64;
    let return_values = vec![Value::byte_array(ByteArray::new(return_val))];
    NativeReturnStatus::Success {
        cost,
        return_values,
    }
}
