// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Defines constants and types that are used throughout cost synthesis.
use vm::file_format::TableIndex;
use vm_runtime_types::value::Value;

/// The maximum number of fields that will be generated for any struct.
pub const MAX_FIELDS: usize = 10;

/// The maximum size that generated byte arrays can be.
pub const BYTE_ARRAY_MAX_SIZE: usize = 64;

/// The maximum size that a generated string can be.
pub const MAX_STRING_SIZE: usize = 32;

/// The maximumm number of locals that can be defined within a generated function definition.
pub const MAX_NUM_LOCALS: usize = 10;

/// The maximum number of arguments to generated function definitions.
pub const MAX_FUNCTION_CALL_SIZE: usize = 8;

/// The maximum number of return types of generated function definitions.
pub const MAX_RETURN_TYPES_LENGTH: usize = 4;

/// The default index to use when we need to have a frame on the execution stack.
///
/// We are always guaranteed to have at least one function definition in a generated module. We can
/// therefore always count on having a function definition at index 0.
pub const DEFAULT_FUNCTION_IDX: TableIndex = 0;

/// The type of the value stack.
pub type Stack = Vec<Value>;
