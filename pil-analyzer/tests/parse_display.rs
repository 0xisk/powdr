use powdr_number::GoldilocksField;
use powdr_pil_analyzer::analyze_string;
use test_log::test;

use pretty_assertions::assert_eq;

#[test]
fn parse_print_analyzed() {
    // This is rather a test for the Display trait than for the analyzer.
    let input = r#"constant %N = 65536;
public P = T.pc(2);
namespace Bin(65536);
    col witness bla;
namespace std::prover(65536);
    let eval: expr -> fe = [];
namespace T(65536);
    col fixed first_step = [1] + [0]*;
    col fixed line(i) { i };
    let ops: int -> bool = (|i| ((i < 7) && (6 >= -i)));
    col witness pc;
    col witness XInv;
    col witness XIsZero;
    T.XIsZero = (1 - (T.X * T.XInv));
    (T.XIsZero * T.X) = 0;
    (T.XIsZero * (1 - T.XIsZero)) = 0;
    col witness instr_jmpz;
    col witness instr_jmpz_param_l;
    col witness instr_jmp;
    col witness instr_jmp_param_l;
    col witness instr_dec_CNT;
    col witness instr_assert_zero;
    (T.instr_assert_zero * (T.XIsZero - 1)) = 0;
    col witness X;
    col witness X_const;
    col witness X_read_free;
    col witness A;
    col witness CNT;
    col witness read_X_A;
    col witness read_X_CNT;
    col witness reg_write_X_CNT;
    col witness read_X_pc;
    col witness reg_write_X_A;
    T.X = ((((T.read_X_A * T.A) + (T.read_X_CNT * T.CNT)) + T.X_const) + (T.X_read_free * T.X_free_value));
    T.A' = (((T.first_step' * 0) + (T.reg_write_X_A * T.X)) + ((1 - (T.first_step' + T.reg_write_X_A)) * T.A));
    col witness X_free_value(__i) query match std::prover::eval(T.pc) { 0 => ("input", 1), 3 => ("input", (std::prover::eval(T.CNT) + 1)), 7 => ("input", 0), };
    col fixed p_X_const = [0, 0, 0, 0, 0, 0, 0, 0, 0] + [0]*;
    col fixed p_X_read_free = [1, 0, 0, 1, 0, 0, 0, -1, 0] + [0]*;
    col fixed p_read_X_A = [0, 0, 0, 1, 0, 0, 0, 1, 1] + [0]*;
    col fixed p_read_X_CNT = [0, 0, 1, 0, 0, 0, 0, 0, 0] + [0]*;
    col fixed p_read_X_pc = [0, 0, 0, 0, 0, 0, 0, 0, 0] + [0]*;
    col fixed p_reg_write_X_A = [0, 0, 0, 1, 0, 0, 0, 1, 0] + [0]*;
    col fixed p_reg_write_X_CNT = [1, 0, 0, 0, 0, 0, 0, 0, 0] + [0]*;
    { T.pc, T.reg_write_X_A, T.reg_write_X_CNT } in (1 - T.first_step) { T.line, T.p_reg_write_X_A, T.p_reg_write_X_CNT };
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(input, formatted);
}

#[test]
fn intermediate() {
    let input = r#"namespace N(65536);
    col witness x;
    col intermediate = x;
    intermediate = intermediate;
"#;
    let expected = r#"namespace N(65536);
    col witness x;
    col intermediate = N.x;
    N.intermediate = N.intermediate;
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, expected);
}

#[test]
fn intermediate_nested() {
    let input = r#"namespace N(65536);
    col witness x;
    col intermediate = x;
    col int2 = intermediate;
    col int3 = int2 + intermediate;
    int3 = 2 * x;
"#;
    let expected = r#"namespace N(65536);
    col witness x;
    col intermediate = N.x;
    col int2 = N.intermediate;
    col int3 = (N.int2 + N.intermediate);
    N.int3 = (2 * N.x);
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, expected);
}

#[test]
fn let_definitions() {
    let input = r#"constant %r = 65536;
namespace N(%r);
    let x;
    let z: int = 2;
    let t: col = |i| i + z;
    let other = [1, z];
    let other_fun: int, fe -> (int, (int -> int)) = |i, j| (i + 7, (|k| k - i));
"#;
    let expected = r#"constant %r = 65536;
namespace N(65536);
    col witness x;
    let z: int = 2;
    col fixed t(i) { (i + N.z) };
    let other: int[] = [1, N.z];
    let other_fun: int, fe -> (int, (int -> int)) = (|i, j| ((i + 7), (|k| (k - i))));
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, expected);
}

#[test]
fn reparse_arrays() {
    let input = r#"public out = N.y[1](2);
namespace N(16);
    col witness y[3];
    (N.y[1] - 2) = 0;
    (N.y[2]' - 2) = 0;
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, input);
}

#[test]
#[should_panic = "Type expr[] does not satisfy trait Sub."]
fn no_direct_array_references() {
    let input = r#"namespace N(16);
    col witness y[3];
    (N.y - 2) = 0;
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, input);
}

#[test]
#[should_panic = "Tried to access element 3 of array of size 3"]
fn no_out_of_bounds() {
    let input = r#"namespace N(16);
    col witness y[3];
    (N.y[3] - 2) = 0;
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, input);
}

#[test]
fn namespaced_call() {
    let input = r#"namespace Assembly(2);
    let A: int -> int = (|i| 0);
    let C = (|i| (Assembly.A((i + 2)) + 3));
    let D = (|i| Assembly.C((i + 3)));
"#;
    let expected = r#"namespace Assembly(2);
    let A: int -> int = (|i| 0);
    let C: int -> int = (|i| (Assembly.A((i + 2)) + 3));
    let D: int -> int = (|i| Assembly.C((i + 3)));
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, expected);
}

#[test]
fn if_expr() {
    let input = r#"namespace Assembly(2);
    col fixed A = [0]*;
    let c = (|i| if (i < 3) { i } else { (i + 9) });
    col fixed D(i) { if (Assembly.c(i) != 0) { 3 } else { 2 } };
"#;
    let expected = r#"namespace Assembly(2);
    col fixed A = [0]*;
    let c: int -> int = (|i| if (i < 3) { i } else { (i + 9) });
    col fixed D(i) { if (Assembly.c(i) != 0) { 3 } else { 2 } };
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, expected);
}

#[test]
fn symbolic_functions() {
    let input = r#"namespace N(16);
    let last_row: int = 15;
    let ISLAST: col = |i| match i { last_row => 1, _ => 0 };
    let x;
    let y;
    let constrain_equal_expr = |A, B| A - B;
    let on_regular_row = |cond| (1 - ISLAST) * cond;
    on_regular_row(constrain_equal_expr(x', y)) = 0;
    on_regular_row(constrain_equal_expr(y', x + y)) = 0;
    "#;
    let expected = r#"namespace N(16);
    let last_row: int = 15;
    col fixed ISLAST(i) { match i { N.last_row => 1, _ => 0, } };
    col witness x;
    col witness y;
    let constrain_equal_expr: expr, expr -> expr = (|A, B| (A - B));
    let on_regular_row: expr -> expr = (|cond| ((1 - N.ISLAST) * cond));
    ((1 - N.ISLAST) * (N.x' - N.y)) = 0;
    ((1 - N.ISLAST) * (N.y' - (N.x + N.y))) = 0;
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, expected);
}

#[test]
fn next_op_on_param() {
    let input = r#"namespace N(16);
    let x;
    let y;
    let next_is_seven = |t| t' - 7;
    next_is_seven(y) = 0;
    "#;
    let expected = r#"namespace N(16);
    col witness x;
    col witness y;
    let next_is_seven: expr -> expr = (|t| (t' - 7));
    (N.y' - 7) = 0;
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, expected);
}

#[test]
fn fixed_symbolic() {
    let input = r#"namespace N(16);
    let last_row = 15;
    let islast = |i| match i { N.last_row => 1, _ => 0, };
    let ISLAST: col = |i| islast(i);
    let x;
    let y;
    x - ISLAST = 0;
    "#;
    let expected = r#"namespace N(16);
    let last_row: int = 15;
    let islast: int -> fe = (|i| match i { N.last_row => 1, _ => 0, });
    col fixed ISLAST(i) { N.islast(i) };
    col witness x;
    col witness y;
    (N.x - N.ISLAST) = 0;
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, expected);
}

#[test]
fn parentheses_lambda() {
    let input = r#"namespace N(16);
    let w = || 2;
    let x: fe = (|i| || w())(w())();
    "#;
    let expected = r#"namespace N(16);
    let w: -> fe = (|| 2);
    constant x = (|i| (|| N.w()))(N.w())();
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, expected);
}

#[test]
fn simple_type_resolution() {
    let input = r#"namespace N(16);
    let w: col[3 + 4];
    "#;
    let expected = r#"namespace N(16);
    col witness w[7];
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, expected);
}

#[test]
fn complex_type_resolution() {
    let input = r#"namespace N(16);
    let f: int -> int = |i| i + 10;
    let x: (int -> int), int -> int = |k, i| k(2**i);
    let y: col[x(f, 2)];
    let z: (((int -> int), int -> int)[], expr) = ([x, x, x, x, x, x, x, x], y[0]);
    "#;
    let expected = r#"namespace N(16);
    let f: int -> int = (|i| (i + 10));
    let x: (int -> int), int -> int = (|k, i| k((2 ** i)));
    col witness y[14];
    let z: (((int -> int), int -> int)[], expr) = ([N.x, N.x, N.x, N.x, N.x, N.x, N.x, N.x], N.y[0]);
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, expected);
}

#[test]
fn function_type_display() {
    let input = r#"namespace N(16);
    let f: (-> int)[] = [(|| 10), (|| 12)];
    let g: (int -> int) -> int = (|f| f(0));
    let h: int -> (int -> int) = (|x| (|i| (x + i)));
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, input);
}

#[test]
fn expr_and_identity() {
    let input = r#"namespace N(16);
    let f: expr, expr -> constr[] = |x, y| [x = y];
    let g: expr -> constr[] = |x| [x = 0];
    let x: col;
    let y: col;
    f(x, y);
    g((x));
    "#;
    let expected = r#"namespace N(16);
    let f: expr, expr -> constr[] = (|x, y| [(x = y)]);
    let g: expr -> constr[] = (|x| [(x = 0)]);
    col witness x;
    col witness y;
    N.x = N.y;
    N.x = 0;
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, expected);
}

#[test]
#[should_panic = "Expected type constr but got type expr"]
fn expression_but_expected_constraint() {
    let input = r#"namespace N(16);
    col witness y;
    (N.y - 2);
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, input);
}

#[test]
#[should_panic = "Expected type: expr\\nInferred type: constr\\n"]
fn constraint_but_expected_expression() {
    let input = r#"namespace N(16);
    col witness y;
    { (N.y - 2) = 0 } in { N.y };
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, input);
}

#[test]
#[should_panic = "Set of declared and used type variables are not the same"]
fn used_undeclared_type_var() {
    let input = r#"let x: T = 8;"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, input);
}

#[test]
#[should_panic = "Set of declared and used type variables are not the same"]
fn declared_unused_type_var() {
    let input = r#"let<T> x: int = 8;"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, input);
}

#[test]
#[should_panic = "Excess type variables in declaration: K\nExcess type variables in type: T"]
fn double_used_undeclared_type_var() {
    let input = r#"let<K> x: T = 8;"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    assert_eq!(formatted, input);
}

#[test]
fn to_expr() {
    let input = r#"
    namespace std::convert(16);
        let expr = [];
    namespace N(16);
        let mul_two: int -> int = |i| i * 2;
        col witness y;
        y = y * std::convert::expr(mul_two(7));
"#;
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    let expected = r#"namespace std::convert(16);
    let expr = [];
namespace N(16);
    let mul_two: int -> int = (|i| (i * 2));
    col witness y;
    N.y = (N.y * 14);
"#;
    assert_eq!(formatted, expected);
}

#[test]
fn col_array_is_array() {
    let input = "
    namespace std::convert(16);
        let expr = [];
    namespace std::array(16);
        let len = [];
    namespace main(16);
        pol commit x1[16];
        let x2: col[16];
        let t: int = std::array::len(x1);
        let r: int = std::array::len(x2);
        x1[0] * std::convert::expr(t) = x2[0] * std::convert::expr(r);
    ";
    let formatted = analyze_string::<GoldilocksField>(input).to_string();
    let expected = r#"namespace std::convert(16);
    let expr = [];
namespace std::array(16);
    let len = [];
namespace main(16);
    col witness x1[16];
    col witness x2[16];
    let t: int = std::array::len::<expr>(main.x1);
    let r: int = std::array::len::<expr>(main.x2);
    (main.x1[0] * 16) = (main.x2[0] * 16);
"#;
    assert_eq!(formatted, expected);
}
