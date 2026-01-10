import time
import random
import math
from typing import Dict, List, Tuple, Any
from prism_finance import Canvas, Var

class Colors:
    """Terminal formatting constants."""
    BOLD = '\033[1m'
    DIM = '\033[2m'
    GREEN = '\033[32m'
    BLUE = '\033[34m'
    YELLOW = '\033[33m'
    CYAN = '\033[36m'
    RED = '\033[31m'
    END = '\033[0m'

class SobolAnalyzer:
    @staticmethod
    def generate_saltelli_scenarios(variables: Dict[Var, Tuple[float, float]], n_samples: int):
        v_list = list(variables.keys())
        k = len(v_list)
        def get_mat(): return [[random.uniform(*variables[v]) for v in v_list] for _ in range(n_samples)]
        a, b = get_mat(), get_mat()
        
        # Block 1: Matrix A
        for r in a: yield {v_list[j]: r[j] for j in range(k)}
        # Block 2: Matrix B
        for r in b: yield {v_list[j]: r[j] for j in range(k)}
        # Block 3 to 3+k: Matrix AB_i
        for i in range(k):
            for r in range(n_samples):
                row = list(a[r])
                row[i] = b[r][i]
                yield {v_list[j]: row[j] for j in range(k)}

    @staticmethod
    def compute_indices(y: List[float], n: int, k: int):
        """
        Uses the Jansen (1999) and Saltelli (2010) estimators for numerical stability.
        """
        y_a = y[:n]
        y_b = y[n:2*n]
        
        # Variance of the output using matrix A as the baseline
        mean_a = sum(y_a) / n
        var_y = sum((val - mean_a)**2 for val in y_a) / (n - 1)
        
        if var_y == 0: return [{"S1": 0.0, "ST": 0.0}] * k

        results = []
        for i in range(k):
            y_ab = y[(2+i)*n : (3+i)*n]
            
            # ST (Total Effect): (1/2N) * sum((Y_A - Y_AB_i)^2) / Var(Y)
            v_ti = sum((y_a[j] - y_ab[j])**2 for j in range(n)) / (2 * n)
            st = v_ti / var_y
            
            # S1 (First Order): 1 - [ (1/2N) * sum((Y_B - Y_AB_i)^2) / Var(Y) ]
            # This is the Jansen estimator, which is more robust to mean-shifts.
            y_b_minus_y_ab_sq = sum((y_b[j] - y_ab[j])**2 for j in range(n)) / (2 * n)
            s1 = 1.0 - (y_b_minus_y_ab_sq / var_y)
            
            # Clamp logical bounds [0, ST]
            s1 = max(0.0, min(st, s1))
            st = max(s1, st)
            
            results.append({"S1": s1, "ST": st})
        return results

def print_sobol_table(var_names: List[str], indices: List[Dict[str, float]]):
    """Prints a high-contrast, sorted sensitivity table."""
    # Combine and sort by Total Effect (ST) descending
    data = sorted(zip(var_names, indices), key=lambda x: x[1]['ST'], reverse=True)
    
    print(f"\n{Colors.BOLD}{'PRISM GLOBAL SENSITIVITY REPORT':^85}{Colors.END}")
    header = f"{'Variable':<20} | {'S1 (Direct)':<25} | {'ST (Total)':<25} | {'Interactivity'}"
    print(f"{Colors.BOLD}{header}{Colors.END}")
    print(f"{Colors.DIM}{'-' * 95}{Colors.END}")

    BAR_WIDTH = 20
    for name, idx in data:
        s1, st = idx['S1'], idx['ST']
        
        # Bar construction
        s1_len = int(s1 * BAR_WIDTH)
        st_len = int(st * BAR_WIDTH)
        gap_len = max(0, st_len - s1_len)
        
        # S1 bar in Green, Interaction portion in Blue
        bar = f"{Colors.GREEN}{'█' * s1_len}{Colors.BLUE}{'█' * gap_len}{Colors.DIM}{'░' * (BAR_WIDTH - st_len)}{Colors.END}"
        
        # Interaction context
        inter_val = st - s1
        inter_tag = f"{Colors.CYAN}{inter_val:>6.1%}{Colors.END}" if inter_val > 0.05 else f"{Colors.DIM}Linear{Colors.END}"
        
        # Noise detection
        if st < 0.01:
            name_str = f"{Colors.DIM}{name:<20}{Colors.END}"
            st_str = f"{Colors.DIM}{st:>7.1%}{Colors.END}"
            s1_str = f"{Colors.DIM}{s1:>7.1%}{Colors.END}"
            inter_tag = f"{Colors.RED}Noise{Colors.END}"
        else:
            name_str = f"{Colors.BOLD}{name:<20}{Colors.END}"
            s1_str = f"{Colors.GREEN}{s1:>7.1%}{Colors.END}"
            st_str = f"{Colors.BLUE}{st:>7.1%}{Colors.END}"

        print(f"{name_str} | {s1_str} {bar} | {st_str} {bar} | {inter_tag}")

    print(f"\n{Colors.DIM}Legend: {Colors.GREEN}█ S1{Colors.END} | {Colors.BLUE}█ ST{Colors.END} | {Colors.CYAN}% Interaction (ST-S1){Colors.END}")

def run_sensitivity_demo():
    MONTHS, N = 120, 5000
    print(f"--- Prism Stress Test: {MONTHS}-Month Portfolio ---")
    
    with Canvas() as model:
        # Define model logic
        capex = Var(2000.0, name="CapEx")
        mkt_price = Var([100.0] * MONTHS, name="MktPrice")
        vol_growth = Var(0.02, name="VolGrowth")
        op_margin = Var(0.35, name="Margin")
        interest_rate = Var(0.06, name="IntRate")
        degradation = Var(0.005, name="Degradation")
        debt_ratio = Var(0.70, name="DebtRatio")

        p_idx = Var([float(i) for i in range(MONTHS)], name="Period")
        volume = 1000 * (1.0 + vol_growth) * (1.0 - (degradation * p_idx))
        revenue = volume * mkt_price
        ebitda = revenue * op_margin
        
        # Temporal debt roll-forward
        debt_balance = ebitda.prev(default=capex * debt_ratio) * 0.98
        net_income = (ebitda - (debt_balance * interest_rate / 12.0)) * 0.75
        terminal_value = net_income.prev(default=0) + net_income

        model.compute_all()
        
        # Define stochastic ranges (+/- 20% of base case)
        v_list = [capex, mkt_price, vol_growth, op_margin, interest_rate, degradation, debt_ratio]
        bounds = {}
        for v in v_list:
            val = v.get_value()
            base = val[-1] if isinstance(val, list) else val
            bounds[v] = (base * 0.8, base * 1.2)

        print(f"Generating Saltelli sequence for {len(bounds)} variables...")
        scenarios = list(SobolAnalyzer.generate_saltelli_scenarios(bounds, N))
        s_map = {f"S_{i:06}": s for i, s in enumerate(scenarios)}
        
        print(f"Evaluating {len(s_map):,} scenarios...")
        start = time.perf_counter()
        results = model.run_batch(s_map, chunk_size=5000)
        dur = time.perf_counter() - start
        print(f"Parallel Execution: {dur:.4f}s ({len(s_map)/dur:,.0f} scenarios/sec)")

        # Extract terminal value (last month of cumulative income)
        ordered_y = [results[f"S_{i:06}"].get(terminal_value)[-1] for i in range(len(s_map))]
        indices = SobolAnalyzer.compute_indices(ordered_y, N, len(v_list))
        
        print_sobol_table([v.name for v in v_list], indices)

if __name__ == "__main__":
    run_sensitivity_demo()