//! Raw FFI bindings for the IPOPT C interface.
//!
//! This module provides a direct, unsafe mapping to the functions and types
//! defined in the `Ipopt/IpoptC.h` header file. The naming conventions
//! (e.g., `CamelCase` for functions, `snake_case` for parameters) are preserved
//! to maintain a one-to-one correspondence with the C API for easier maintenance
//! and cross-referencing.

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use libc::{c_char, c_int, c_void};

// --- Type Aliases and Constants ---

/// The integer type used by IPOPT for array indexing and counts.
pub type Index = c_int;
/// The floating-point type used by IPOPT for all numerical values.
pub type Number = f64;
/// The boolean type used by IPOPT, where non-zero is true.
pub type Bool = c_int;

/// An opaque pointer to the internal IPOPT problem structure.
pub type IpoptProblem = *mut c_void;

/// Represents negative infinity for IPOPT bounds.
pub const IPOPT_NEGINF: Number = -1.0e19;
/// Represents positive infinity for IPOPT bounds.
pub const IPOPT_POSINF: Number = 1.0e19;

// --- Callback Function Pointer Types ---
// These define the signatures of the Rust functions that will be called back by IPOPT
// during the optimization process.

/// Callback for evaluating the objective function, `f(x)`.
pub type Eval_F_CB = extern "C" fn(
    n: Index,
    x: *mut Number,
    new_x: Bool,
    obj_value: *mut Number,
    user_data: *mut c_void,
) -> Bool;

/// Callback for evaluating the constraint functions, `g(x)`.
pub type Eval_G_CB = extern "C" fn(
    n: Index,
    x: *mut Number,
    new_x: Bool,
    m: Index,
    g: *mut Number,
    user_data: *mut c_void,
) -> Bool;

/// Callback for evaluating the gradient of the objective function, `∇f(x)`.
pub type Eval_Grad_F_CB = extern "C" fn(
    n: Index,
    x: *mut Number,
    new_x: Bool,
    grad_f: *mut Number,
    user_data: *mut c_void,
) -> Bool;

/// Callback for evaluating the Jacobian of the constraint functions, `∇g(x)`.
/// This function provides both the sparsity structure and the values of the Jacobian.
pub type Eval_Jac_G_CB = extern "C" fn(
    n: Index,
    x: *mut Number,
    new_x: Bool,
    m: Index,
    nele_jac: Index,
    iRow: *mut Index,
    jCol: *mut Index,
    values: *mut Number,
    user_data: *mut c_void,
) -> Bool;

/// Callback for evaluating the Hessian of the Lagrangian, `∇²L(x, σ_f, λ)`.
/// This can be omitted if a quasi-Newton approximation is used.
pub type Eval_H_CB = extern "C" fn(
    n: Index,
    x: *mut Number,
    new_x: Bool,
    obj_factor: Number,
    m: Index,
    lambda: *mut Number,
    new_lambda: Bool,
    nele_hess: Index,
    iRow: *mut Index,
    jCol: *mut Index,
    values: *mut Number,
    user_data: *mut c_void,
) -> Bool;

/// An optional callback executed at the end of each solver iteration.
pub type Intermediate_CB = extern "C" fn(
    alg_mod: Index,
    iter_count: Index,
    obj_value: Number,
    inf_pr: Number,
    inf_du: Number,
    mu: Number,
    d_norm: Number,
    regularization_size: Number,
    alpha_du: Number,
    alpha_pr: Number,
    ls_trials: Index,
    user_data: *mut c_void,
) -> Bool;

// --- IPOPT Enums ---

/// The final status of the optimization attempt.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationReturnStatus {
    Solve_Succeeded = 0,
    Solved_To_Acceptable_Level = 1,
    Infeasible_Problem_Detected = 2,
    Search_Direction_Becomes_Too_Small = 3,
    Diverging_Iterates = 4,
    User_Requested_Stop = 5,
    Feasible_Point_Found = 6,
    Maximum_Iterations_Exceeded = -1,
    Restoration_Failed = -2,
    Error_In_Step_Computation = -3,
    Maximum_CpuTime_Exceeded = -4,
    Not_Enough_Degrees_Of_Freedom = -10,
    Invalid_Problem_Definition = -11,
    Invalid_Option = -12,
    Invalid_Number_Detected = -13,
    Unrecoverable_Exception = -100,
    NonIpopt_Exception_Thrown = -101,
    Insufficient_Memory = -102,
    Internal_Error = -199,
}

/// Specifies the indexing style (0-based or 1-based) for matrix structures.
#[repr(C)]
pub enum IndexStyle {
    C_STYLE = 0,
    FORTRAN_STYLE = 1,
}

pub use IndexStyle::C_STYLE as FR_C_STYLE;

// --- Core IPOPT API Functions ---

#[link(name = "ipopt")]
extern "C" {
    /// Creates a new IPOPT problem instance.
    pub fn CreateIpoptProblem(
        n: Index,
        x_L: *mut Number,
        x_U: *mut Number,
        m: Index,
        g_L: *mut Number,
        g_U: *mut Number,
        nele_jac: Index,
        nele_hess: Index,
        index_style: IndexStyle,
        eval_f: Option<Eval_F_CB>,
        eval_g: Option<Eval_G_CB>,
        eval_grad_f: Option<Eval_Grad_F_CB>,
        eval_jac_g: Option<Eval_Jac_G_CB>,
        eval_h: Option<Eval_H_CB>,
        user_data: *mut c_void,
    ) -> IpoptProblem;

    /// Frees the memory associated with an IPOPT problem instance.
    pub fn FreeIpoptProblem(ipopt_problem: IpoptProblem);

    /// Sets the intermediate callback function.
    pub fn SetIntermediateCallback(
        ipopt_problem: IpoptProblem,
        intermediate_cb: Option<Intermediate_CB>,
    ) -> Bool;

    // --- Option Setting Functions ---

    /// Sets a string-valued option.
    pub fn AddIpoptStrOption(
        ipopt_problem: IpoptProblem,
        keyword: *const c_char,
        val: *const c_char,
    ) -> Bool;

    /// Sets a numeric (floating-point) option.
    pub fn AddIpoptNumOption(ipopt_problem: IpoptProblem, keyword: *const c_char, val: Number) -> Bool;

    /// Sets an integer-valued option.
    pub fn AddIpoptIntOption(ipopt_problem: IpoptProblem, keyword: *const c_char, val: c_int) -> Bool;

    /// The main function to run the optimization.
    pub fn IpoptSolve(
        ipopt_problem: IpoptProblem,
        x: *mut Number,
        g: *mut Number,
        obj_val: *mut Number,
        mult_g: *mut Number,
        mult_x_L: *mut Number,
        mult_x_U: *mut Number,
        user_data: *mut c_void,
    ) -> ApplicationReturnStatus;
}