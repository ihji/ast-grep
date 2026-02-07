use anyhow::{anyhow, Result};
use std::fmt::Debug;

pub fn with_ctx<T, E>(result: std::result::Result<T, E>, context: &str) -> Result<T>
where
  E: Debug,
{
  result.map_err(|err| anyhow!("{context}: {:?}", err))
}

pub fn transpose_with_ctx<T, E>(
  maybe_result: Option<std::result::Result<T, E>>,
  context: &str,
) -> Result<Option<T>>
where
  E: Debug,
{
  maybe_result
    .transpose()
    .map_err(|err| anyhow!("{context}: {:?}", err))
}

pub fn map_transposed_or_default<T, E, U, F>(
  maybe_result: Option<std::result::Result<T, E>>,
  context: &str,
  default: U,
  map: F,
) -> Result<U>
where
  E: Debug,
  F: FnOnce(T) -> Result<U>,
{
  transpose_with_ctx(maybe_result, context)?
    .map(map)
    .transpose()
    .map(|value| value.unwrap_or(default))
}

pub fn flatten_nested<T>(nested: Vec<Vec<T>>) -> Vec<T> {
  nested.into_iter().flatten().collect()
}
