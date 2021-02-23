// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use diem_json_rpc_types::views::{MoveAbortExplanationView, VMStatusView};
use diem_types::vm_status::{AbortLocation, KeptVMStatus};

/// Helper macros. Used to simplify adding new RpcHandler to Registry
/// `registry` - name of local registry variable
/// `name`  - name for the rpc method
/// `method` - method name of new rpc method
/// `required_num_args` - number of required method arguments
/// `opt_num_args` - number of optional method arguments
macro_rules! register_rpc_method {
    ($registry:expr, $name: expr, $method: expr, $required_num_args: expr, $opt_num_args: expr) => {
        $registry.insert(
            $name.to_string(),
            Box::new(move |service, request| {
                Box::pin(async move {
                    if request.params.len() < $required_num_args
                        || request.params.len() > $required_num_args + $opt_num_args
                    {
                        let expected = if $opt_num_args == 0 {
                            format!("{}", $required_num_args)
                        } else {
                            format!(
                                "{}..{}",
                                $required_num_args,
                                $required_num_args + $opt_num_args
                            )
                        };
                        anyhow::bail!(JsonRpcError::invalid_params_size(format!(
                            "wrong number of arguments (given {}, expected {})",
                            request.params.len(),
                            expected,
                        )));
                    }

                    fail_point!(format!("jsonrpc::method::{}", $name).as_str(), |_| {
                        Err(anyhow::format_err!("Injected error for method {} error", $name).into())
                    });
                    Ok(serde_json::to_value($method(service, request).await?)?)
                })
            }),
        );
    };
}

pub fn vm_status_view_from_kept_vm_status(status: &KeptVMStatus) -> VMStatusView {
    match status {
        KeptVMStatus::Executed => VMStatusView::Executed,
        KeptVMStatus::OutOfGas => VMStatusView::OutOfGas,
        KeptVMStatus::MoveAbort(loc, abort_code) => {
            let explanation = if let AbortLocation::Module(module_id) = loc {
                move_explain::get_explanation(module_id, *abort_code).map(|context| {
                    MoveAbortExplanationView {
                        category: context.category.code_name,
                        category_description: context.category.code_description,
                        reason: context.reason.code_name,
                        reason_description: context.reason.code_description,
                    }
                })
            } else {
                None
            };

            VMStatusView::MoveAbort {
                explanation,
                location: loc.to_string(),
                abort_code: *abort_code,
            }
        }
        KeptVMStatus::ExecutionFailure {
            location,
            function,
            code_offset,
        } => VMStatusView::ExecutionFailure {
            location: location.to_string(),
            function_index: *function,
            code_offset: *code_offset,
        },
        KeptVMStatus::MiscellaneousError => VMStatusView::MiscellaneousError,
    }
}
