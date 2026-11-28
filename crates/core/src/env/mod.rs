//! Pure domain environment variable resolution, candidate discovery, and
//! `{{var}}` template interpolation (`zest_core::env`).

pub mod interpolation;
pub mod resolve;

pub use interpolation::{
    interpolate, interpolate_cow, interpolate_with_map, interpolate_with_vars, scan_variable_spans,
    scan_variables, InterpolationResult, VariableInterpolator, VariableSpan,
};
pub use resolve::{
    find_variable_span_at, list_candidates, merge_effective_environment, resolve_variable,
    ResolvedVariable, VariableCandidate, VariableScope, SECRET_MASK,
};
