//! Implements the argmin::core traits for the PrismProblem.

use crate::computation::ComputationError;
use crate::solver::problem::PrismProblem;
use argmin::core::{CostFunction, Error, Executor, Gradient, Hessian, State};
use argmin::solver::trustregion::{Dogleg, TrustRegion};
use nalgebra::{DMatrix, DVector};
use std::sync::Arc;

/// Implements the cost function for the solver.
/// The cost is the sum of the squares of the constraint residuals.
impl CostFunction for PrismProblem<'_> {
    type Param = DVector<f64>;
    type Output = f64;

    fn cost(&self, p: &Self::Param) -> Result<Self::Output, Error> {
        let mut ledger = self.base_ledger.clone();

        // 1. Set the current guess for solver variables in the ledger.
        for (i, var_id) in self.variables.iter().enumerate() {
            // Assuming all solver vars are scalars for now.
            ledger.insert(*var_id, Ok(Arc::new(vec![p[i]])));
        }

        // 2. Compute the values of the residual nodes based on the current guess.
        self.sync_engine
            .compute(&self.residuals, &mut ledger)
            .map_err(|e| Error::msg(e.to_string()))?;

        // 3. Calculate the cost: sum of squares of the residuals.
        let mut sum_sq: f64 = 0.0;
        for residual_id in &self.residuals {
            if let Some(Ok(val)) = ledger.get(*residual_id) {
                // Assuming residuals are also scalars.
                sum_sq += val.get(0).unwrap_or(&0.0).powi(2);
            } else {
                let msg = format!("Failed to compute residual for node {}", residual_id.index());
                return Err(Error::msg(msg));
            }
        }

        Ok(sum_sq)
    }
}

/// Implements the gradient using central-difference finite differences.
impl Gradient for PrismProblem<'_> {
    type Param = DVector<f64>;
    type Gradient = DVector<f64>;

    fn gradient(&self, p: &Self::Param) -> Result<Self::Gradient, Error> {
        // Use the free function from finitediff instead of an extension method.
        let f = |x: &DVector<f64>| self.cost(x).unwrap();
        Ok(finitediff::central::gradient(p, f))
    }
}

/// Implements the Hessian using central-difference finite differences.
impl Hessian for PrismProblem<'_> {
    type Param = DVector<f64>;
    type Hessian = DMatrix<f64>;

    fn hessian(&self, p: &Self::Param) -> Result<Self::Hessian, Error> {
        // Use the free function from finitediff.
        let f = |x: &DVector<f64>| self.cost(x).unwrap();
        Ok(finitediff::central::hessian(p, f))
    }
}

/// The main entry point for the solver.
pub fn solve(problem: PrismProblem) -> Result<crate::computation::Ledger, ComputationError> {
    let num_vars = problem.variables.len();
    if num_vars == 0 {
        return Ok(problem.base_ledger);
    }
    
    // Define an initial guess for the parameters (solver variables).
    let init_param = DVector::from_vec(vec![0.0; num_vars]);

    // Instantiate the Dogleg method as the subproblem solver.
    let dogleg: Dogleg<f64> = Dogleg::new();

    // Instantiate the main TrustRegion solver, providing it the Dogleg solver.
    let solver: TrustRegion<Dogleg<f64>, f64> = TrustRegion::new(dogleg);

    // Run the optimization.
    let res = Executor::new(problem, solver)
        .configure(|state| {
            state
                .param(init_param)
                .max_iters(100)
                .target_cost(1e-12) // Stop when sum of squares is very close to zero.
        })
        .run()
        .map_err(|e| ComputationError::SolverDidNotConverge(e.to_string()))?;

    // Check for convergence to a valid solution (cost must be near zero).
    let final_cost = res.state().get_best_cost();
    if final_cost > 1e-9 {
        return Err(ComputationError::SolverDidNotConverge(format!(
            "Solver finished but final residual error was high: {:.2e}",
            final_cost.sqrt()
        )));
    }

    // Extract the problem and the solution from the final state.
    let problem_ref = res.state().get_problem().unwrap();
    let best_params = res.state().get_best_param().unwrap();

    // Create the final ledger with the solved values.
    let mut final_ledger = problem_ref.base_ledger.clone();
    for (i, var_id) in problem_ref.variables.iter().enumerate() {
        final_ledger.insert(*var_id, Ok(Arc::new(vec![best_params[i]])));
    }

    Ok(final_ledger)
}