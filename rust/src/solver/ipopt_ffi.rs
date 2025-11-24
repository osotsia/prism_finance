#![allow(non_camel_case_types, non_snake_case, dead_code)]
use libc::{c_char, c_int, c_void};

pub type Index = c_int;
pub type Number = f64;
pub type Bool = c_int;
pub type IpoptProblem = *mut c_void;

pub const IPOPT_NEGINF: Number = -1.0e19;
pub const IPOPT_POSINF: Number = 1.0e19;

pub type Eval_F_CB = extern "C" fn(Index, *mut Number, Bool, *mut Number, *mut c_void) -> Bool;
pub type Eval_G_CB = extern "C" fn(Index, *mut Number, Bool, Index, *mut Number, *mut c_void) -> Bool;
pub type Eval_Grad_F_CB = extern "C" fn(Index, *mut Number, Bool, *mut Number, *mut c_void) -> Bool;
pub type Eval_Jac_G_CB = extern "C" fn(Index, *mut Number, Bool, Index, Index, *mut Index, *mut Index, *mut Number, *mut c_void) -> Bool;
pub type Eval_H_CB = extern "C" fn(Index, *mut Number, Bool, Number, Index, *mut Number, Bool, Index, *mut Index, *mut Index, *mut Number, *mut c_void) -> Bool;
pub type Intermediate_CB = extern "C" fn(Index, Index, Number, Number, Number, Number, Number, Number, Number, Number, Index, *mut c_void) -> Bool;

#[repr(C)]
pub enum IndexStyle { C_STYLE = 0, FORTRAN_STYLE = 1 }
pub use IndexStyle::C_STYLE as FR_C_STYLE;

#[link(name = "ipopt")]
extern "C" {
    pub fn CreateIpoptProblem(
        n: Index, x_L: *mut Number, x_U: *mut Number, m: Index, g_L: *mut Number, g_U: *mut Number,
        nele_jac: Index, nele_hess: Index, index_style: IndexStyle,
        eval_f: Option<Eval_F_CB>, eval_g: Option<Eval_G_CB>, eval_grad_f: Option<Eval_Grad_F_CB>,
        eval_jac_g: Option<Eval_Jac_G_CB>, eval_h: Option<Eval_H_CB>, user_data: *mut c_void
    ) -> IpoptProblem;
    pub fn FreeIpoptProblem(ipopt_problem: IpoptProblem);
    pub fn AddIpoptStrOption(ipopt_problem: IpoptProblem, keyword: *const c_char, val: *const c_char) -> Bool;
    pub fn AddIpoptNumOption(ipopt_problem: IpoptProblem, keyword: *const c_char, val: Number) -> Bool;
    pub fn AddIpoptIntOption(ipopt_problem: IpoptProblem, keyword: *const c_char, val: c_int) -> Bool;
    pub fn IpoptSolve(
        ipopt_problem: IpoptProblem, x: *mut Number, g: *mut Number, obj_val: *mut Number,
        mult_g: *mut Number, mult_x_L: *mut Number, mult_x_U: *mut Number, user_data: *mut c_void
    ) -> c_int;
    pub fn SetIntermediateCallback(ipopt_problem: IpoptProblem, cb: Option<Intermediate_CB>) -> Bool;
}