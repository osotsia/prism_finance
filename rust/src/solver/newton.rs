//! Implements the `argmin::core::Operator` trait for Newton's method using finite differences.
use crate::solver::problem::SolverProblem;
use crate::computation::ComputationError;
use argmin::core::{CostFunction, Error, Executor, Gradient, Hessian, State};
use argmin::solver::trustregion::NewtonTR;
use argmin_math::nalgebra::{DMatrix, DVector};
use petgraph::Direction;
use std::sync::Arc;

impl CostFunction for SolverProblem<'_> {
    type Param = DVector<f64>;
    type Output = f64;

    fn cost(&self, p: &Self::Param) -> Result<Self::Output, Error> {
        let mut ledger = self.base_ledger.clone();
        for (i, var_id) in self.variables.iter().enumerate() {
            ledger.insert(*var_id, Ok(Arc::new(vec![p[i]])));
        }
        self.sync_engine.compute(&self.constraints, &mut ledger)
            .map_err(|e| Error::Msg(e.to_string()))?;
        
        let mut sum_sq: f64 = 0.0;
        for constraint_id in &self.constraints {
            let residual_id = self.graph.graph.neighbors_directed(*constraint_id, Direction::Incoming).next()
                .ok_or_else(|| Error::Msg(format!("Constraint node {} has no residual parent", constraint_id.index())))?;
            
            if let Some(Ok(val)) = ledger.get(residual_id) {
                sum_sq += val.get(0).unwrap_or(&0.0).powi(2);
            } else {
                return Err(Error::Msg(format!("Failed to compute residual for constraint {}", constraint_id.index())));
            }
        }
        Ok(sum_sq)
    }
}

impl Gradient for SolverProblem<'_> {
    type Param = DVector<f64>;
    type Gradient = DVector<f64>;
    fn gradient(&self, p: &Self::Param) -> Result<Self::Gradient, Error> {
        argmin_math::finitediff::forward_grad(self, p)
    }
}

impl Hessian for SolverProblem<'_> {
    type Param = DVector<f64>;
    type Hessian = DMatrix<f64>;
    fn hessian(&self, p: &Self::Param) -> Result<Self::Hessian, Error> {
        argmin_math::finitediff::forward_hessian(&|x| self.gradient(x), p)
    }
}

pub fn solve(problem: SolverProblem) -> Result<crate::computation::Ledger, ComputationError> {
    let init_param = DVector::from_vec(vec![0.0; problem.variables.len()]);
    let solver = NewtonTR::new();
    let res = Executor::new(problem, solver)
        .configure(|state| state.param(init_param).max_iters(100).target_cost(1e-9))
        .run()
        .map_err(|e| ComputationError::SolverDidNotConverge(e.to_string()))?;
    
    let problem_ref = res.state.problem.as_ref().unwrap();
    let mut final_ledger = problem_ref.base_ledger.clone();
    let best_params = res.state.best_param;

    for (i, var_id) in problem_ref.variables.iter().enumerate() {
        final_ledger.insert(*var_id, Ok(Arc::new(vec![best_params[i]])));
    }
    Ok(final_ledger)
}