// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use libra_types::vm_status::sub_status::NFE_LCS_SERIALIZATION_FAILURE;
use move_vm_types::{
    gas_schedule::NativeCostIndex,
    loaded_data::runtime_types::Type,
    natives::function::{native_gas, NativeContext, NativeResult},
    values::{values_impl::Reference, Value},
};
use std::collections::VecDeque;
use vm::errors::PartialVMResult;
use move_core_types::account_address::AccountAddress;

/// Rust implementation of Move's `native public fun to_bytes<T>(&T): vector<u8>`
pub fn native_to_bytes(
    context: &mut impl NativeContext,
    mut ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 1);
    debug_assert!(args.len() == 1);

    let ref_to_val = pop_arg!(args, Reference);

    let arg_type = ty_args.pop().unwrap();
    // delegate to the LCS serialization for `Value`
    let serialized_value_opt = match context.type_to_type_layout(&arg_type)? {
        None => None,
        Some(layout) => ref_to_val.read_ref()?.simple_serialize(&layout),
    };
    let serialized_value = match serialized_value_opt {
        None => {
            let cost = native_gas(context.cost_table(), NativeCostIndex::LCS_TO_BYTES, 1);
            return Ok(NativeResult::err(cost, NFE_LCS_SERIALIZATION_FAILURE));
        }
        Some(serialized_value) => serialized_value,
    };

    // cost is proportional to the size of the serialized value
    let cost = native_gas(
        context.cost_table(),
        NativeCostIndex::LCS_TO_BYTES,
        serialized_value.len(),
    );

    Ok(NativeResult::ok(
        cost,
        vec![Value::vector_u8(serialized_value)],
    ))
}

/// Rust implementation of Move's `public fun from_public_key_vec(pub_key_vec: vector<u8>): address;`
pub fn native_from_public_key_vec(
    context: &mut impl NativeContext,
    mut _ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(_ty_args.is_empty());
    debug_assert!(args.len() == 1);

    let key_bytes = pop_arg!(args, Vec<u8>);

    let mut array = [0u8; AccountAddress::LENGTH];
    array.copy_from_slice(&key_bytes[AccountAddress::LENGTH..]);
    let address =  AccountAddress::new(array);

    let cost = native_gas(
        context.cost_table(),
        NativeCostIndex::LCS_FROM_PUBLIC_KEY,
        key_bytes.len(),
    );

    let return_values = vec![Value::address(address)];
    Ok(NativeResult::ok(cost, return_values))
}

