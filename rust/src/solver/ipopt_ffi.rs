//! Raw FFI bindings for the IPOPT C interface.
//!
//! These definitions are adapted from the `Ipopt/IpoptC.h` header file.

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use libc::{c_char, c_int, c_void};

pub type Index = c_int;
pub type Number = f64;
pub type Bool = c_int; // IPOPT's C interface uses 'int' as its boolean type.

/// A pointer to the opaque IPOPT problem structure.
pub type IpoptProblem = *mut c_void;

pub const IPOPT_NEGINF: Number = -1.0e19;
pub const IPOPT_POSINF: Number = 1.0e19;

// --- Callback Function Pointer Types ---

pub type Eval_F_CB = extern "C" fn(
    n: Index,
    x: *mut Number,
    new_x: Bool,
    obj_value: *mut Number,
    user_data: *mut c_void,
) -> Bool;

pub type Eval_G_CB = extern "C" fn(
    n: Index,
    x: *mut Number,
    new_x: Bool,
    m: Index,
    g: *mut Number,
    user_data: *mut c_void,
) -> Bool;

pub type Eval_Grad_F_CB = extern "C" fn(
    n: Index,
    x: *mut Number,
    new_x: Bool,
    grad_f: *mut Number,
    user_data: *mut c_void,
) -> Bool;

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

// --- Enum for return status ---
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

// --- Enum for Jacobian/Hessian structure ---
#[repr(C)]
pub enum IndexStyle {
    C_STYLE = 0,
    FORTRAN_STYLE = 1,
}
pub use IndexStyle::C_STYLE as FR_C_STYLE;

// --- Core IPOPT API Functions ---
#[link(name = "ipopt")]
extern "C" {
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

    pub fn FreeIpoptProblem(ipopt_problem: IpoptProblem);

    pub fn SetIntermediateCallback(
        ipopt_problem: IpoptProblem,
        intermediate_cb: Option<Intermediate_CB>,
    ) -> Bool;

    pub fn AddIpoptStrOption(
        ipopt_problem: IpoptProblem,
        keyword: *const c_char,
        val: *const c_char,
    ) -> Bool;
    pub fn AddIpoptNumOption(ipopt_problem: IpoptProblem, keyword: *const c_char, val: Number)
        -> Bool;
    pub fn AddIpoptIntOption(ipopt_problem: IpoptProblem, keyword: *const c_char, val: c_int)
        -> Bool;

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