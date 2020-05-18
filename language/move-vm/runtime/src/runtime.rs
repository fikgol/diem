// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    data_cache::{RemoteCache, TransactionDataCache},
    interpreter::Interpreter,
    loader::Loader,
    session::Session,
};
use libra_logger::prelude::*;
use move_core_types::{
    account_address::AccountAddress,
    identifier::IdentStr,
    language_storage::{ModuleId, TypeTag},
    vm_status::StatusCode,
};
use move_vm_types::{data_store::DataStore, gas_schedule::CostStrategy, values::Value};
use vm::{
    errors::{Location, PartialVMError, PartialVMResult, VMResult},
    file_format::SignatureToken,
    CompiledModule,
};

/// An instantiation of the MoveVM.
pub(crate) struct VMRuntime {
    loader: Loader,
}

impl VMRuntime {
    pub(crate) fn new() -> Self {
        VMRuntime {
            loader: Loader::new(),
        }
    }

    pub fn new_session<'r, R: RemoteCache>(&self, remote: &'r R) -> Session<'r, '_, R> {
        Session {
            runtime: self,
            data_cache: TransactionDataCache::new(remote, &self.loader),
        }
    }

    pub(crate) fn publish_module(
        &self,
        module: Vec<u8>,
        _sender: AccountAddress,
        data_store: &mut impl DataStore,
        _cost_strategy: &mut CostStrategy,
    ) -> VMResult<()> {
        // deserialize the module. Perform bounds check. After this indexes can be
        // used with the `[]` operator
        let compiled_module = match CompiledModule::deserialize(&module) {
            Ok(module) => module,
            Err(err) => {
                warn!("[VM] module deserialization failed {:?}", err);
                return Err(err.finish(Location::Undefined));
            }
        };

        let module_id = compiled_module.self_id();

        // perform bytecode and loading verification
        self.loader.verify_module(&compiled_module)?;

        data_store.publish_module(&module_id, module)
    }

    pub(crate) fn execute_script(
        &self,
        script: Vec<u8>,
        ty_args: Vec<TypeTag>,
        mut args: Vec<Value>,
        senders: Vec<AccountAddress>,
        data_store: &mut impl DataStore,
        cost_strategy: &mut CostStrategy,
    ) -> VMResult<()> {
        // signer helper closure
        fn is_signer_reference(s: &SignatureToken) -> bool {
            use SignatureToken as S;
            match s {
                S::Reference(inner) => matches!(&**inner, S::Signer),
                _ => false,
            }
        }

        // load the script, perform verification
        let (main, type_params) = self.loader.load_script(&script, &ty_args, data_store)?;

        // Build the arguments list for the main and check the arguments are of restricted types.
        // Signers are built up from left-to-right. Either all signer arguments are used, or no
        // signer arguments can be be used by a script.
        let parameters = &main.parameters().0;
        let first_param_is_signer_ref = parameters.get(0).map_or(false, is_signer_reference);
        if first_param_is_signer_ref {
            if parameters.len() != args.len() + senders.len() {
                return Err(PartialVMError::new(StatusCode::TYPE_MISMATCH)
                    .with_message("Scripts must use all or no signers".to_string())
                    .finish(Location::Script));
            }
            senders.into_iter().for_each(|addr| {
                args.insert(0, Value::transaction_argument_signer_reference(addr))
            });
        }

        check_args(&args).map_err(|e| e.finish(Location::Script))?;

        // run the script
        Interpreter::entrypoint(
            main,
            type_params,
            args,
            data_store,
            cost_strategy,
            &self.loader,
        )
    }

    pub(crate) fn execute_function(
        &self,
        module: &ModuleId,
        function_name: &IdentStr,
        ty_args: Vec<TypeTag>,
        args: Vec<Value>,
        data_store: &mut impl DataStore,
        cost_strategy: &mut CostStrategy,
    ) -> VMResult<()> {
        // load the function in the given module, perform verification of the module and
        // its dependencies if the module was not loaded
        let (func, type_params) =
            self.loader
                .load_function(function_name, module, &ty_args, data_store)?;

        // check the arguments provided are of restricted types
        check_args(&args).map_err(|e| e.finish(Location::Module(module.clone())))?;

        // run the function
        Interpreter::entrypoint(
            func,
            type_params,
            args,
            data_store,
            cost_strategy,
            &self.loader,
        )
    }

    pub(crate) fn loader(&self) -> &Loader {
        &self.loader
    }
}

// Check that the transaction arguments are acceptable by the VM.
// Constants and a reference to a `Signer` are the only arguments allowed.
// This check is more of a rough filter to remove obvious bad arguments.
fn check_args(args: &[Value]) -> PartialVMResult<()> {
    for val in args {
        if !val.is_constant_or_signer_ref() {
            return Err(PartialVMError::new(StatusCode::TYPE_MISMATCH)
                .with_message("VM argument types are restricted".to_string()));
        }
    }
    Ok(())
}
