use std::{collections::BTreeMap, iter::repeat};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use evalexpr::{build_operator_tree, ContextWithMutableVariables, HashMapContext, Node, Value};
use exmex::{parse_with_default_ops, BinOp, FlatEx, Operator};
use fasteval::{Compiler, Evaler, Instruction, Slab};
use itertools::{izip, Itertools};

use rsc::{
    computer::Computer,
    lexer::tokenize,
    parser::{parse, Expr},
};
const N: usize = 4;

const BENCH_EXPRESSIONS_NAMES: [&str; N] = ["sin", "power", "nested", "compile"];
const BENCH_EXPRESSIONS_STRS: [&str; N] = [
    "sin(x)+sin(y)+sin(z)",
    "x^2+y*y+z^z",
    "x*0.02*sin(-(3*(2*sin(x-1/(sin(y*5)+(5.0-1/z))))))",
    "x*0.2*5/4+x*2*4*1*1*1*1*1*1*1+7*sin(y)-z/sin(3.0/2/(1-x*4*1*1*1*1))",
];

const BENCH_EXPRESSIONS_REFS: [fn(f64, f64, f64) -> f64; N] = [
    |x, y, z| x.sin() + y.sin() + z.sin(),
    |x, y, z| x.powi(2) + y * y + z.powf(z),
    |x, y, z| {
        x * 0.02 * (-(3.0 * (2.0 * (x - 1.0 / ((y * 5.0).sin() + (5.0 - 1.0 / z))).sin()))).sin()
    },
    |x, y, z| {
        x * 0.2 * 5.0 / 4.0 + x * 2.0 * 4.0 + 7.0 * y.sin()
            - z / (3.0 / 2.0 / (1.0 - x * 4.0f64)).sin()
    },
];
const BENCH_X_RANGE: (usize, usize) = (0, 5);
const BENCH_Y: f64 = 3.0;
const BENCH_Z: f64 = 4.0;

fn bench_ref_values() -> Vec<Vec<f64>> {
    BENCH_EXPRESSIONS_REFS
        .iter()
        .map(|f| {
            (BENCH_X_RANGE.0..BENCH_X_RANGE.1)
                .map(|i| f(i as f64, BENCH_Y, BENCH_Z))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
}

fn assert_float_eq(f1: f64, f2: f64) {
    assert!((f1 - f2).abs() <= 1e-12);
}

fn run_benchmark<F: FnMut(f64) -> f64>(funcs: Vec<F>, eval_name: &str, c: &mut Criterion) {
    for (mut func, exp_name, ref_vals) in izip!(
        funcs,
        BENCH_EXPRESSIONS_NAMES.iter(),
        bench_ref_values().iter()
    ) {
        c.bench_function(format!("{}_{}", eval_name, exp_name).as_str(), |b| {
            b.iter(|| {
                for (i, ref_val) in izip!(BENCH_X_RANGE.0..BENCH_X_RANGE.1, ref_vals) {
                    assert_float_eq(func(black_box(i as f64)), *ref_val);
                }
            })
        });
    }
}

fn run_benchmark_parse<'a, T, F: Fn(&'a [&str]) -> Vec<T>>(
    func: F,
    parse_name: &str,
    c: &mut Criterion,
) {
    c.bench_function(parse_name.to_string().as_str(), |b| {
        b.iter(|| {
            func(black_box(&BENCH_EXPRESSIONS_STRS));
        })
    });
}

fn exmex_parse<'a>(strings: &'a [&str]) -> Vec<FlatEx<'a, f64>> {
    strings
        .iter()
        .map(|expr_str| parse_with_default_ops::<f64>(expr_str).unwrap())
        .collect::<Vec<_>>()
}

fn exmex_bench_parse(c: &mut Criterion) {
    run_benchmark_parse(exmex_parse, "exmex_parse", c);
}

fn exmex_parse_optimized<'a>(strings: &'a [&str]) -> Vec<FlatEx<'a, f64>> {
    let ops = vec![
        Operator {
            repr: "^",
            bin_op: Some(BinOp {
                apply: |a: f64, b| a.powf(b),
                prio: 2,
            }),
            unary_op: None,
        },
        Operator {
            repr: "*",
            bin_op: Some(BinOp {
                apply: |a, b| a * b,
                prio: 1,
            }),
            unary_op: None,
        },
        Operator {
            repr: "/",
            bin_op: Some(BinOp {
                apply: |a, b| a / b,
                prio: 1,
            }),
            unary_op: None,
        },
        Operator {
            repr: "+",
            bin_op: Some(BinOp {
                apply: |a, b| a + b,
                prio: 0,
            }),
            unary_op: Some(|a| a),
        },
        Operator {
            repr: "-",
            bin_op: Some(BinOp {
                apply: |a, b| a - b,
                prio: 0,
            }),
            unary_op: Some(|a| (-a)),
        },
        Operator {
            repr: "sin",
            bin_op: None,
            unary_op: Some(|a| a.sin()),
        },
    ];
    strings
        .iter()
        .map(|expr_str| exmex::parse(expr_str, &ops).unwrap())
        .collect::<Vec<_>>()
}

fn exmex_bench_parse_optimized(c: &mut Criterion) {
    run_benchmark_parse(exmex_parse_optimized, "exmex_parse_optimized", c);
}

fn exmex_bench_eval(c: &mut Criterion) {
    let parsed_exprs = exmex_parse(&BENCH_EXPRESSIONS_STRS);
    let funcs = parsed_exprs
        .iter()
        .map(|expr| move |x: f64| expr.eval(&[x, BENCH_Y, BENCH_Z]).unwrap())
        .collect::<Vec<_>>();
    run_benchmark(funcs, "exmex", c);
}

fn evalexpr_parse(strings: &[&str]) -> Vec<(Node, HashMapContext)> {
    let parsed_exprs = strings.iter().map(|expr_str| {
        build_operator_tree(expr_str.replace("sin", "math::sin").as_str()).unwrap()
    });
    let contexts = repeat(HashMapContext::new()).take(N);
    izip!(parsed_exprs, contexts).collect_vec()
}

fn evalexpr_bench_parse(c: &mut Criterion) {
    run_benchmark_parse(evalexpr_parse, "evalexpr_parse", c);
}

fn evalexpr_bench_eval(c: &mut Criterion) {
    let mut parsed_exprs = evalexpr_parse(&BENCH_EXPRESSIONS_STRS);
    let funcs = parsed_exprs
        .iter_mut()
        .map(|(expr, context)| {
            move |x: f64| {
                context.set_value("x".into(), x.into()).unwrap();
                context.set_value("y".into(), BENCH_Y.into()).unwrap();
                context.set_value("z".into(), BENCH_Z.into()).unwrap();
                match expr.eval_with_context(context).unwrap() {
                    Value::Float(val) => val,
                    _ => panic!("What?"),
                }
            }
        })
        .collect::<Vec<_>>();
    run_benchmark(funcs, "evalexpr", c);
}

fn meval_parse(strings: &[&str]) -> Vec<impl Fn(f64, f64, f64) -> f64> {
    strings
        .iter()
        .map(|expr_str| {
            let expr = expr_str.parse::<meval::Expr>().unwrap();
            expr.bind3("x", "y", "z").unwrap()
        })
        .collect::<Vec<_>>()
}

fn meval_bench_parse(c: &mut Criterion) {
    run_benchmark_parse(meval_parse, "meval_parse", c);
}

fn meval_bench_eval(c: &mut Criterion) {
    let parsed_exprs = meval_parse(&BENCH_EXPRESSIONS_STRS);
    let funcs = parsed_exprs
        .iter()
        .map(|expr| move |x: f64| expr(x, BENCH_Y, BENCH_Z))
        .collect::<Vec<_>>();
    run_benchmark(funcs, "meval", c);
}

fn fasteval_parse(strings: &[&str]) -> Vec<((Instruction, Slab), BTreeMap<String, f64>)> {
    let parsed_exprs = strings.iter().map(|expr_str| {
        let parser = fasteval::Parser::new();
        let mut slab = fasteval::Slab::new();
        (
            parser
                .parse(expr_str, &mut slab.ps)
                .unwrap()
                .from(&slab.ps)
                .compile(&slab.ps, &mut slab.cs),
            slab,
        )
    });
    let contexts = repeat(BTreeMap::<String, f64>::new()).take(N);
    izip!(parsed_exprs, contexts).collect::<Vec<_>>()
}

fn fasteval_bench_parse(c: &mut Criterion) {
    run_benchmark_parse(fasteval_parse, "fasteval_parse", c);
}
fn fasteval_bench_eval(c: &mut Criterion) {
    let mut parsed_exprs = fasteval_parse(&BENCH_EXPRESSIONS_STRS);
    let funcs = parsed_exprs
        .iter_mut()
        .map(|tuple_of_tuples| {
            let context = &mut tuple_of_tuples.1;
            let (instr, slab) = &tuple_of_tuples.0;
            move |x: f64| {
                context.insert("x".to_string(), x);
                context.insert("y".to_string(), BENCH_Y);
                context.insert("z".to_string(), BENCH_Z);
                || -> Result<f64, fasteval::Error> {
                    Ok(fasteval::eval_compiled_ref!(
                        instr,
                        black_box(slab),
                        context
                    ))
                }()
                .unwrap()
            }
        })
        .collect::<Vec<_>>();
    run_benchmark(funcs, "fasteval", c);
}

fn rsc_parse<'a>(strings: &[&str]) -> Vec<(Expr<f64>, Computer<'a, f64>)> {
    let parsed_exprs = strings.iter().map(|expr_str| {
        let tokens = tokenize(expr_str, true).unwrap();
        parse(&tokens).unwrap()
    });
    let computers = repeat(Computer::<f64>::default()).take(N);
    izip!(parsed_exprs, computers).collect_vec()
}
fn rsc_bench_parse(c: &mut Criterion) {
    run_benchmark_parse(rsc_parse, "rsc_parse", c);
}

fn rsc_bench_eval(c: &mut Criterion) {
    let mut parsed_exprs = rsc_parse(&BENCH_EXPRESSIONS_STRS);
    let funcs = parsed_exprs
        .iter_mut()
        .map(|(ast, comp)| {
            move |x: f64| {
                let mut ast = ast.clone();
                ast.replace(&Expr::Identifier("x".to_owned()), &Expr::Constant(x), false);
                ast.replace(
                    &Expr::Identifier("y".to_owned()),
                    &Expr::Constant(BENCH_Y),
                    false,
                );
                ast.replace(
                    &Expr::Identifier("z".to_owned()),
                    &Expr::Constant(BENCH_Z),
                    false,
                );
                comp.compute(&ast).unwrap()
            }
        })
        .collect::<Vec<_>>();
    run_benchmark(funcs, "rsc", c);
}
criterion_group!(
    benches,
    fasteval_bench_eval,
    exmex_bench_eval,
    meval_bench_eval,
    rsc_bench_eval,
    evalexpr_bench_eval,
    fasteval_bench_parse,
    exmex_bench_parse,
    exmex_bench_parse_optimized,
    meval_bench_parse,
    rsc_bench_parse,
    evalexpr_bench_parse,
);
criterion_main!(benches);
