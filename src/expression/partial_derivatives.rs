use num::Float;
use smallvec::{smallvec, SmallVec};
use std::fmt::Debug;

use super::{
    deep::{BinOpsWithReprs, DeepEx, ExprIdxVec},
    deep_details::{self, find_overloaded_ops, OverloadedOps},
};
use crate::{
    definitions::N_BINOPS_OF_DEEPEX_ON_STACK,
    expression::deep::{DeepNode, UnaryOpWithReprs},
    operators::{Operator, UnaryOp},
    ExParseError,
};

#[derive(Clone)]
struct ValueDerivative<'a, T: Copy + Debug> {
    val: DeepEx<'a, T>,
    der: DeepEx<'a, T>,
}

pub fn find_op<'a, T: Copy + Debug>(
    repr: &'a str,
    ops: &[Operator<'a, T>],
) -> Option<Operator<'a, T>> {
    let found = ops.iter().cloned().find(|op| op.repr == repr);
    match found {
        Some(op) => Some(Operator {
            bin_op: op.bin_op,
            unary_op: op.unary_op,
            repr: repr,
        }),
        None => None,
    }
}

#[derive(Debug)]
pub struct PartialDerivative<'a, T: Copy + Debug> {
    repr: &'a str,
    bin_op: Option<
        fn(
            ValueDerivative<'a, T>,
            ValueDerivative<'a, T>,
            &'a [Operator<'a, T>],
        ) -> Result<ValueDerivative<'a, T>, ExParseError>,
    >,
    unary_op:
        Option<fn(DeepEx<'a, T>, &'a [Operator<'a, T>]) -> Result<DeepEx<'a, T>, ExParseError>>,
}

fn find_as_bin_op_with_reprs<'a, T: Copy + Debug>(
    repr: &'a str,
    ops: &[Operator<'a, T>],
) -> Result<BinOpsWithReprs<'a, T>, ExParseError> {
    let op = find_op(repr, ops).ok_or(ExParseError {
        msg: format!("did not find operator {}", repr),
    })?;
    Ok(BinOpsWithReprs {
        reprs: vec![op.repr],
        ops: smallvec![op.bin_op.ok_or(ExParseError {
            msg: format!("operater {} is not binary", op.repr)
        })?],
    })
}

fn find_as_unary_op_with_reprs<'a, T: Copy + Debug>(
    repr: &'a str,
    ops: &[Operator<'a, T>],
) -> Result<UnaryOpWithReprs<'a, T>, ExParseError> {
    let op = find_op(repr, ops).ok_or(ExParseError {
        msg: format!("did not find operator {}", repr),
    })?;
    Ok(UnaryOpWithReprs {
        reprs: vec![op.repr],
        op: UnaryOp::from_vec(smallvec![op.unary_op.ok_or(ExParseError {
            msg: format!("operater {} is not unary", op.repr)
        })?]),
    })
}

fn make_op_missing_err(repr: &str) -> ExParseError {
    ExParseError {
        msg: format!("operator {} needed for outer partial derivative", repr),
    }
}

fn partial_derivative_outer<'a, T: Float + Debug>(
    deepex: DeepEx<'a, T>,
    partial_derivative_ops: &[PartialDerivative<'a, T>],
    overloaded_ops: OverloadedOps<'a, T>,
    ops: &'a [Operator<'a, T>],
) -> Result<DeepEx<'a, T>, ExParseError> {
    let factorexes =
        deepex
            .unary_op()
            .reprs
            .iter()
            .map(|repr| -> Result<DeepEx<'a, T>, ExParseError> {
                let op = partial_derivative_ops
                    .iter()
                    .find(|pdo| &pdo.repr == repr)
                    .ok_or(make_op_missing_err(repr))?;
                let unary_op = op.unary_op.clone().ok_or(make_op_missing_err(repr))?;

                unary_op(deepex.clone(), ops)
            });
    let resex = factorexes.fold(
        Ok(DeepEx::one(overloaded_ops)),
        |dp1, dp2| -> Result<DeepEx<T>, ExParseError> { Ok(mul_num(dp1?, dp2?)?) },
    );
    resex
}

fn partial_derivative_inner<'a, T: Float + Debug>(
    var_idx: usize,
    deepex: DeepEx<'a, T>,
    partial_derivative_ops: &[PartialDerivative<'a, T>],
    overloaded_ops: OverloadedOps<'a, T>,
    ops: &'a [Operator<'a, T>],
) -> Result<DeepEx<'a, T>, ExParseError> {
    // special case, partial derivative of only 1 node
    if deepex.nodes().len() == 1 {
        match deepex.nodes()[0].clone() {
            DeepNode::Num(_) => return Ok(DeepEx::zero(overloaded_ops.clone())),
            DeepNode::Var((var_i, _)) => {
                return if var_i == var_idx {
                    Ok(DeepEx::one(overloaded_ops.clone()))
                } else {
                    Ok(DeepEx::zero(overloaded_ops.clone()))
                };
            }
            DeepNode::Expr(mut e) => {
                e.set_overloaded_ops(Some(overloaded_ops.clone()));
                return partial_deepex(var_idx, e, ops);
            }
        }
    }

    let prio_indices = deep_details::prioritized_indices(&deepex.bin_ops().ops, deepex.nodes());

    let make_deepex = |node: DeepNode<'a, T>| match node {
        DeepNode::Expr(mut e) => {
            e.set_overloaded_ops(Some(overloaded_ops.clone()));
            e
        }
        _ => DeepEx::from_node(node, overloaded_ops.clone()),
    };

    let mut nodes = deepex
        .nodes()
        .iter()
        .map(|node| -> Result<_, ExParseError> {
            let deepex_val = make_deepex(node.clone());
            let deepex_der = partial_deepex(var_idx, deepex_val.clone(), ops)?;
            Ok(Some(ValueDerivative {
                val: deepex_val,
                der: deepex_der,
            }))
        })
        .collect::<Result<Vec<_>, ExParseError>>()?;

    let partial_bin_ops_of_deepex =
        deepex
            .bin_ops()
            .reprs
            .iter()
            .map(|repr| -> Result<&PartialDerivative<'a, T>, ExParseError> {
                partial_derivative_ops
                    .iter()
                    .find(|pdo| &pdo.repr == repr)
                    .ok_or(ExParseError {
                        msg: format!(
                            "derivative operator of {} needed for partial derivative",
                            repr
                        ),
                    })
            })
            .collect::<Result<
                SmallVec<[&PartialDerivative<'a, T>; N_BINOPS_OF_DEEPEX_ON_STACK]>,
                ExParseError,
            >>()?;

    let mut num_inds = prio_indices.clone();
    let mut used_prio_indices = ExprIdxVec::new();

    for (i, &bin_op_idx) in prio_indices.iter().enumerate() {
        let num_idx = num_inds[i];
        let node_1 = nodes[num_idx].take();
        let node_2 = nodes[num_idx + 1].take();

        let pd_deepex = if let (Some(n1), Some(n2)) = (node_1, node_2) {
            let pdo = &partial_bin_ops_of_deepex[bin_op_idx];
            (pdo.bin_op.unwrap())(n1, n2, ops)
        } else {
            Err(ExParseError {
                msg: "nodes do not contain values in partial derivative".to_string(),
            })
        }?;
        nodes[num_idx] = Some(pd_deepex);
        nodes.remove(num_idx + 1);
        // reduce indices after removed position
        for num_idx_after in num_inds.iter_mut() {
            if *num_idx_after > num_idx {
                *num_idx_after = *num_idx_after - 1;
            }
        }
        used_prio_indices.push(bin_op_idx);
    }
    let mut res = nodes[0]
        .take()
        .ok_or(ExParseError {
            msg: "node 0 needs to contain valder at the end of partial derviative".to_string(),
        })?
        .der;
    res.set_overloaded_ops(Some(overloaded_ops));
    Ok(res)
}

pub fn partial_deepex<'a, T: Float + Debug + 'a>(
    var_idx: usize,
    deepex: DeepEx<'a, T>,
    ops: &'a [Operator<'a, T>],
) -> Result<DeepEx<'a, T>, ExParseError> {
    let partial_derivative_ops = make_partial_derivative_ops::<T>();
    let overloaded_ops = find_overloaded_ops(ops).ok_or(ExParseError {
        msg: "one of overloaded ops not found".to_string(),
    })?;

    let inner = partial_derivative_inner(
        var_idx,
        deepex.clone(),
        &partial_derivative_ops,
        overloaded_ops.clone(),
        ops,
    )?;
    let outer =
        partial_derivative_outer(deepex, &partial_derivative_ops, overloaded_ops.clone(), ops)?;
    let mut res = mul_num(inner, outer)?;
    res.compile();
    res.set_overloaded_ops(Some(overloaded_ops));
    Ok(res)
}


fn add_num<'a, T: Float + Debug>(
    summand_1: DeepEx<'a, T>,
    summand_2: DeepEx<'a, T>,
) -> Result<DeepEx<'a, T>, ExParseError> {
    let (summand_1, summand_2) = summand_1.var_names_union(summand_2);
    Ok(if summand_1.is_zero() {
        summand_2
    } else if summand_2.is_zero() {
        summand_1
    } else {
        summand_1 + summand_2
    })
}

fn mul_num<'a, T: Float + Debug>(
    factor_1: DeepEx<'a, T>,
    factor_2: DeepEx<'a, T>,
) -> Result<DeepEx<'a, T>, ExParseError> {
    let zero = DeepEx::zero(factor_1.unpack_and_clone_overloaded_ops()?);
    let (factor_1, factor_2) = factor_1.var_names_union(factor_2);
    let zero = zero.var_names_like_other(&factor_1);
    Ok(if factor_1.is_zero() || factor_2.is_zero() {
        zero
    } else if factor_1.is_one() {
        factor_2
    } else if factor_2.is_one() {
        factor_1
    } else {
        factor_1 * factor_2
    })
}

fn pow_num<'a, T: Float + Debug>(
    base: DeepEx<'a, T>,
    exponent: DeepEx<'a, T>,
    power_op: BinOpsWithReprs<'a, T>,
) -> Result<DeepEx<'a, T>, ExParseError> {
    let zero = DeepEx::zero(base.unpack_and_clone_overloaded_ops()?);
    let one = DeepEx::one(base.unpack_and_clone_overloaded_ops()?);
    let (base, exponent) = base.var_names_union(exponent);
    let zero = zero.var_names_like_other(&base);
    let one = one.var_names_like_other(&base);
    Ok(if base.is_zero() && exponent.is_zero() {
        Err(ExParseError {
            msg: "base and exponent both zero. help. fatal. ah. help.".to_string(),
        })?
    } else if base.is_zero() {
        zero
    } else if exponent.is_zero() {
        one
    } else {
        base.operate_bin(exponent, power_op)
    })
}

pub fn make_partial_derivative_ops<'a, T: Float + Debug>() -> Vec<PartialDerivative<'a, T>> {
    vec![
        PartialDerivative {
            repr: "^",
            bin_op: Some(
                |f: ValueDerivative<T>,
                 g: ValueDerivative<T>,
                 ops: &'a [Operator<'a, T>]|
                 -> Result<ValueDerivative<T>, ExParseError> {
                    let power_op = find_as_bin_op_with_reprs("^", ops)?;
                    let log_op = find_as_unary_op_with_reprs("log", ops)?;

                    let one = DeepEx::one(f.val.unpack_and_clone_overloaded_ops()?);
                    let val = pow_num(f.val.clone(), g.val.clone(), power_op.clone())?;

                    let der_1 = mul_num(
                        mul_num(
                            pow_num(f.val.clone(), g.val.clone() - one, power_op.clone())?,
                            g.val.clone(),
                        )?,
                        f.der.clone(),
                    )?;

                    let der_2 = mul_num(
                        mul_num(val.clone(), f.val.operate_unary(log_op))?,
                        g.der.clone(),
                    )?;

                    let der = add_num(der_1, der_2)?;
                    Ok(ValueDerivative { val: val, der: der })
                },
            ),
            unary_op: None,
        },
        PartialDerivative {
            repr: "+",
            bin_op: Some(
                |f: ValueDerivative<T>,
                 g: ValueDerivative<T>,
                 _: &'a [Operator<'a, T>]|
                 -> Result<ValueDerivative<T>, ExParseError> {
                    Ok(ValueDerivative {
                        val: add_num(f.val, g.val)?,
                        der: add_num(f.der, g.der)?,
                    })
                },
            ),
            unary_op: Some(
                |f: DeepEx<T>, _: &'a [Operator<'a, T>]| -> Result<DeepEx<T>, ExParseError> {
                    Ok(f.clone())
                },
            ),
        },
        PartialDerivative {
            repr: "*",
            bin_op: Some(
                |f: ValueDerivative<T>,
                 g: ValueDerivative<T>,
                 _: &'a [Operator<'a, T>]|
                 -> Result<ValueDerivative<T>, ExParseError> {
                    let val = mul_num(f.val.clone(), g.val.clone())?;

                    let der_1 = mul_num(g.val, f.der)?;
                    let der_2 = mul_num(g.der, f.val)?;
                    let der = add_num(der_1, der_2)?;
                    Ok(ValueDerivative { val: val, der: der })
                },
            ),
            unary_op: None,
        },
        PartialDerivative {
            repr: "sin",
            bin_op: None,
            unary_op: Some(
                |f: DeepEx<T>, ops: &'a [Operator<'a, T>]| -> Result<DeepEx<T>, ExParseError> {
                    let unary_op = find_as_unary_op_with_reprs("cos", ops)?;
                    Ok(f.with_new_unary_op(unary_op))
                },
            ),
        },
        PartialDerivative {
            repr: "cos",
            bin_op: None,
            unary_op: Some(
                |f: DeepEx<T>, ops: &'a [Operator<'a, T>]| -> Result<DeepEx<T>, ExParseError> {
                    let mut unary_op = find_as_unary_op_with_reprs("sin", ops)?;
                    let mut minus = find_as_unary_op_with_reprs("-", ops)?;
                    unary_op.append_front(&mut minus);
                    Ok(f.with_new_unary_op(unary_op))
                },
            ),
        },
        PartialDerivative {
            repr: "-",
            bin_op: None,
            unary_op: Some(
                |f: DeepEx<'a, T>,
                 ops: &'a [Operator<'a, T>]|
                 -> Result<DeepEx<'a, T>, ExParseError> {
                    let minus = find_as_unary_op_with_reprs("-", ops)?;
                    Ok(f.with_new_unary_op(minus))
                },
            ),
        },
        PartialDerivative {
            repr: "log",
            bin_op: None,
            unary_op: Some(
                |f: DeepEx<'a, T>,
                 _: &'a [Operator<'a, T>]|
                 -> Result<DeepEx<'a, T>, ExParseError> {
                    Ok(DeepEx::one(f.unpack_and_clone_overloaded_ops()?) / f)
                },
            ),
        },
    ]
}

#[cfg(test)]
use {
    super::flat::flatten,
    crate::{operators::make_default_operators, util::assert_float_eq_f64},
};

#[test]
fn test_partial_x2x() {
    let ops = make_default_operators::<f64>();
    let deepex = DeepEx::<f64>::from_str("x * 2 * x").unwrap();
    let derivative = partial_deepex(0, deepex.clone(), &ops).unwrap();
    let result = flatten(derivative.clone()).eval(&[0.0]).unwrap();
    assert_float_eq_f64(result, 0.0);
    let result = flatten(derivative).eval(&[1.0]).unwrap();
    assert_float_eq_f64(result, 4.0);
}

#[test]
fn test_partial_cos_squared() {
    let ops = make_default_operators::<f64>();
    let deepex = DeepEx::<f64>::from_str("cos(y) ^ 2").unwrap();
    let derivative = partial_deepex(0, deepex.clone(), &ops).unwrap();
    let result = flatten(derivative.clone()).eval(&[0.0]).unwrap();
    assert_float_eq_f64(result, 0.0);
    let result = flatten(derivative).eval(&[1.0]).unwrap();
    assert_float_eq_f64(result, -0.9092974268256818);
}

#[test]
fn test_partial_combined() {
    let ops = make_default_operators::<f64>();
    let deepex = DeepEx::<f64>::from_str("sin(x) + cos(y) ^ 2").unwrap();
    let derivative = partial_deepex(1, deepex.clone(), &ops).unwrap();
    let result = flatten(derivative.clone()).eval(&[231.431, 0.0]).unwrap();
    assert_float_eq_f64(result, 0.0);
    let result = flatten(derivative).eval(&[-12.0, 1.0]).unwrap();
    assert_float_eq_f64(result, -0.9092974268256818);
}

#[test]
fn test_partial_derivative_second_var() {
    let ops = make_default_operators::<f64>();
    let deepex = DeepEx::<f64>::from_str("sin(x) + cos(y)").unwrap();
    let derivative = partial_deepex(1, deepex.clone(), &ops).unwrap();
    let result = flatten(derivative.clone()).eval(&[231.431, 0.0]).unwrap();
    assert_float_eq_f64(result, 0.0);
    let result = flatten(derivative).eval(&[-12.0, 1.0]).unwrap();
    assert_float_eq_f64(result, -0.8414709848078965);
}

#[test]
fn test_partial_derivative_first_var() {
    let ops = make_default_operators::<f64>();

    let deepex = DeepEx::<f64>::from_str("sin(x) + cos(y)").unwrap();
    let derivative = partial_deepex(0, deepex.clone(), &ops).unwrap();
    let result = flatten(derivative.clone()).eval(&[0.0, 2345.03]).unwrap();
    assert_float_eq_f64(result, 1.0);
    let result = flatten(derivative).eval(&[1.0, 43212.43]).unwrap();
    assert_float_eq_f64(result, 0.5403023058681398);
}

#[test]
fn test_partial_outer() {
    let partial_derivative_ops = make_partial_derivative_ops::<f64>();
    let ops = make_default_operators::<f64>();

    let deepex_1 = DeepEx::<f64>::from_str("sin(x)").unwrap();
    let deepex = deepex_1.nodes()[0].clone();

    match deepex {
        DeepNode::Expr(e) => {
            let deri = partial_derivative_outer(
                e,
                &partial_derivative_ops,
                deepex_1.overloaded_ops().clone().unwrap(),
                &ops,
            )
            .unwrap();
            assert_eq!(deri.nodes().len(), 2);
            let flatex = flatten(deri);
            assert_float_eq_f64(flatex.eval(&[1.0]).unwrap(), 0.5403023058681398);
            assert_float_eq_f64(flatex.eval(&[0.0]).unwrap(), 1.0);
            assert_float_eq_f64(flatex.eval(&[2.0]).unwrap(), -0.4161468365471424);
        }
        _ => (),
    }
}

#[test]
fn test_partial_derivative_simple() {
    let ops = make_default_operators::<f64>();

    let deepex = DeepEx::<f64>::from_str("1").unwrap();
    let derivative = partial_deepex(0, deepex, &ops).unwrap();

    assert_eq!(derivative.nodes().len(), 1);
    assert_eq!(derivative.bin_ops().ops.len(), 0);
    match derivative.nodes()[0] {
        DeepNode::Num(n) => assert_float_eq_f64(n, 0.0),
        _ => assert!(false),
    }
    let deepex = DeepEx::<f64>::from_str("x").unwrap();
    let derivative = partial_deepex(0, deepex, &ops).unwrap();
    assert_eq!(derivative.nodes().len(), 1);
    assert_eq!(derivative.bin_ops().ops.len(), 0);
    match derivative.nodes()[0] {
        DeepNode::Num(n) => assert_float_eq_f64(n, 1.0),
        _ => assert!(false),
    }
    let deepex = DeepEx::<f64>::from_str("x^2").unwrap();
    let derivative = partial_deepex(0, deepex, &ops).unwrap();
    let result = flatten(derivative).eval(&[4.5]).unwrap();
    assert_float_eq_f64(result, 9.0);

    let deepex = DeepEx::<f64>::from_str("sin(x)").unwrap();

    let derivative = partial_deepex(0, deepex.clone(), &ops).unwrap();
    let result = flatten(derivative.clone()).eval(&[0.0]).unwrap();
    assert_float_eq_f64(result, 1.0);
    let result = flatten(derivative).eval(&[1.0]).unwrap();
    assert_float_eq_f64(result, 0.5403023058681398);
}
