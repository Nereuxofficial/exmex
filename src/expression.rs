use crate::{
    operators::{BinOp, UnaryOp},
    ExParseError, Operator,
};
use smallvec::{smallvec, SmallVec};
use std::{
    fmt,
    fmt::{Debug, Display, Formatter},
    iter::repeat,
    ops::{Add, Div, Mul, Sub},
};

type ExprIdxVec = SmallVec<[usize; 32]>;

/// Container of binary operators of one expression.
pub type BinOpVec<T> = SmallVec<[BinOp<T>; 32]>;

pub const N_NODES_ON_STACK: usize = 32usize;

const ADD_REPR: &str = "+";
const SUB_REPR: &str = "-";
const MUL_REPR: &str = "*";
const DIV_REPR: &str = "/";
const POW_REPR: &str = "^";

pub type FlatNodeVec<T> = SmallVec<[FlatNode<T>; N_NODES_ON_STACK]>;
pub type FlatOpVec<T> = SmallVec<[FlatOp<T>; N_NODES_ON_STACK]>;

/// A `FlatOp` contains besides a binary operation an optional unary operation that
/// will be executed after the binary operation in case of its existence.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct FlatOp<T: Copy> {
    unary_op: UnaryOp<T>,
    bin_op: BinOp<T>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum FlatNodeKind<T: Copy> {
    Num(T),
    Var(usize),
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct FlatNode<T: Copy> {
    kind: FlatNodeKind<T>,
    unary_op: UnaryOp<T>,
}

impl<T: Copy> FlatNode<T> {
    pub fn from_kind(kind: FlatNodeKind<T>) -> FlatNode<T> {
        return FlatNode {
            kind: kind,
            unary_op: UnaryOp::new(),
        };
    }
}

fn flatten_vecs<T: Copy + Debug>(
    deep_expr: &DeepEx<T>,
    prio_offset: i32,
) -> (FlatNodeVec<T>, FlatOpVec<T>) {
    let mut flat_nodes = FlatNodeVec::<T>::new();
    let mut flat_ops = FlatOpVec::<T>::new();

    for (node_idx, node) in deep_expr.nodes.iter().enumerate() {
        match node {
            DeepNode::Num(num) => {
                let flat_node = FlatNode::from_kind(FlatNodeKind::Num(*num));
                flat_nodes.push(flat_node);
            }
            DeepNode::Var(idx) => {
                let flat_node = FlatNode::from_kind(FlatNodeKind::Var(*idx));
                flat_nodes.push(flat_node);
            }
            DeepNode::Expr(e) => {
                let (mut sub_nodes, mut sub_ops) = flatten_vecs(e, prio_offset + 100i32);
                flat_nodes.append(&mut sub_nodes);
                flat_ops.append(&mut sub_ops);
            }
        };
        if node_idx < deep_expr.bin_ops.ops.len() {
            let prio_adapted_bin_op = BinOp {
                apply: deep_expr.bin_ops.ops[node_idx].apply,
                prio: deep_expr.bin_ops.ops[node_idx].prio + prio_offset,
            };
            flat_ops.push(FlatOp {
                bin_op: prio_adapted_bin_op,
                unary_op: UnaryOp::new(),
            });
        }
    }

    if deep_expr.unary_op.op.len() > 0 {
        if flat_ops.len() > 0 {
            // find the last binary operator with the lowest priority of this expression,
            // since this will be executed as the last one
            let low_prio_op = match flat_ops.iter_mut().rev().min_by_key(|op| op.bin_op.prio) {
                None => panic!("cannot have more than one flat node but no binary ops"),
                Some(x) => x,
            };
            low_prio_op
                .unary_op
                .append_front(&mut deep_expr.unary_op.op.clone());
        } else {
            flat_nodes[0]
                .unary_op
                .append_front(&mut deep_expr.unary_op.op.clone());
        }
    }
    (flat_nodes, flat_ops)
}

pub fn flatten<T: Copy + Debug>(deepex: DeepEx<T>) -> FlatEx<T> {
    let (nodes, ops) = flatten_vecs(&deepex, 0);
    let indices = prioritized_indices_flat(&ops, &nodes);
    let mut found_vars = SmallVec::<[usize; 16]>::new();
    let n_unique_vars = nodes
        .iter()
        .filter_map(|n| match n.kind {
            FlatNodeKind::Var(idx) => {
                if !found_vars.contains(&idx) {
                    found_vars.push(idx);
                    Some(idx)
                } else {
                    None
                }
            }
            _ => None,
        })
        .count();
    FlatEx {
        nodes: nodes,
        ops: ops,
        prio_indices: indices,
        n_unique_vars: n_unique_vars,
        deepex: Some(deepex),
    }
}

fn prioritized_indices_flat<T: Copy>(ops: &[FlatOp<T>], nodes: &FlatNodeVec<T>) -> ExprIdxVec {
    let prio_increase =
        |bin_op_idx: usize| match (&nodes[bin_op_idx].kind, &nodes[bin_op_idx + 1].kind) {
            (FlatNodeKind::Num(_), FlatNodeKind::Num(_)) => {
                let prio_inc = 5;
                &ops[bin_op_idx].bin_op.prio * 10 + prio_inc
            }
            _ => &ops[bin_op_idx].bin_op.prio * 10,
        };
    let mut indices: ExprIdxVec = (0..ops.len()).collect();
    indices.sort_by(|i1, i2| {
        let prio_i1 = prio_increase(*i1);
        let prio_i2 = prio_increase(*i2);
        prio_i2.partial_cmp(&prio_i1).unwrap()
    });
    indices
}

fn prioritized_indices<T: Copy + Debug>(bin_ops: &[BinOp<T>], nodes: &[DeepNode<T>]) -> ExprIdxVec {
    let prio_increase = |bin_op_idx: usize| match (&nodes[bin_op_idx], &nodes[bin_op_idx + 1]) {
        (DeepNode::Num(_), DeepNode::Num(_)) => {
            let prio_inc = 5;
            &bin_ops[bin_op_idx].prio * 10 + prio_inc
        }
        _ => &bin_ops[bin_op_idx].prio * 10,
    };

    let mut indices: ExprIdxVec = (0..bin_ops.len()).collect();
    indices.sort_by(|i1, i2| {
        let prio_i1 = prio_increase(*i1);
        let prio_i2 = prio_increase(*i2);
        prio_i2.partial_cmp(&prio_i1).unwrap()
    });
    indices
}

pub fn find_overloaded_ops<'a, T: Copy>(
    all_ops: &[Operator<'a, T>],
) -> Result<OverloadedOps<'a, T>, ExParseError> {
    let find_op = |repr| all_ops.iter().cloned().find(|op| op.repr == repr);

    let make_err = |repr| ExParseError {
        msg: format!("did not find overloaded operator {}", repr),
    };

    Ok(OverloadedOps {
        add: find_op(ADD_REPR).ok_or(make_err(ADD_REPR))?,
        sub: find_op(SUB_REPR).ok_or(make_err(SUB_REPR))?,
        mul: find_op(MUL_REPR).ok_or(make_err(MUL_REPR))?,
        div: find_op(DIV_REPR).ok_or(make_err(DIV_REPR))?,
        pow: find_op(POW_REPR).ok_or(make_err(POW_REPR))?,
    })
}

/// This is the core data type representing a flattened expression and the result of
/// parsing a string. We use flattened expressions to make efficient evaluation possible.
/// Simplified, a flat expression consists of a [`SmallVec`](SmallVec) of nodes and a
/// [`SmallVec`](SmallVec) of operators that are applied to the nodes in an order following
/// operator priorities.
///
/// You create an expression with the `parse` function or one of its
/// variants, namely `parse_with_default_ops` and `parse_with_number_pattern`.
///
/// ```rust
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// #
/// use exmex::{parse_with_default_ops};
///
/// // create an expression by parsing a string
/// let expr = parse_with_default_ops::<f32>("sin(1+y)*x")?;
/// assert!((expr.eval(&[2.0, 1.5])? - (1.0 + 2.0 as f32).sin() * 1.5).abs() < 1e-6);
/// #
/// #     Ok(())
/// # }
/// ```
/// The second argument `&[2.0, 1.5]` in the call of [`eval`](FlatEx::eval) specifies the
/// variable values in the order of their occurrence in the string.
/// In this example, we want to evaluate the expression for the varibale values `y=2.0` and `x=1.5`.
/// Variables in the string to-be-parsed are all substrings that are no numbers, no
/// operators, and no parentheses.
///
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct FlatEx<'a, T: Copy + Debug> {
    nodes: FlatNodeVec<T>,
    ops: FlatOpVec<T>,
    prio_indices: ExprIdxVec,
    n_unique_vars: usize,
    deepex: Option<DeepEx<'a, T>>,
}

impl<'a, T: Copy + Debug> FlatEx<'a, T> {
    /// Evaluates an expression with the given variable values and returns the computed
    /// result.
    ///
    /// # Arguments
    ///
    /// * `vars` - Values of the variables of the expression; the n-th value corresponds to
    ///            the n-th variable as given in the string that has been parsed to this expression.
    ///            Thereby, only the first occurrence of the variable in the string is relevant.
    ///
    /// # Errors
    ///
    /// If the number of variables in the parsed expression are different from the length of
    /// the variable slice, we return an [`ExParseError`](ExParseError).
    ///
    pub fn eval(&self, vars: &[T]) -> Result<T, ExParseError> {
        if self.n_unique_vars != vars.len() {
            return Err(ExParseError {
                msg: format!(
                    "parsed expression contains {} vars but passed slice has {} elements",
                    self.n_unique_vars,
                    vars.len()
                ),
            });
        }
        let mut numbers = self
            .nodes
            .iter()
            .map(|node| {
                node.unary_op.apply(match node.kind {
                    FlatNodeKind::Num(n) => n,
                    FlatNodeKind::Var(idx) => vars[idx],
                })
            })
            .collect::<SmallVec<[T; 32]>>();
        let mut ignore: SmallVec<[bool; N_NODES_ON_STACK]> = smallvec![false; N_NODES_ON_STACK];
        for (i, &bin_op_idx) in self.prio_indices.iter().enumerate() {
            let num_idx = self.prio_indices[i];
            let mut shift_left = 0usize;
            while ignore[num_idx - shift_left] {
                shift_left += 1usize;
            }
            let mut shift_right = 1usize;
            while ignore[num_idx + shift_right] {
                shift_right += 1usize;
            }
            let num_1 = numbers[num_idx - shift_left];
            let num_2 = numbers[num_idx + shift_right];
            numbers[num_idx - shift_left] = {
                let bop_res = (self.ops[bin_op_idx].bin_op.apply)(num_1, num_2);
                self.ops[bin_op_idx].unary_op.apply(bop_res)
            };
            ignore[num_idx + shift_right] = true;
        }
        Ok(numbers[0])
    }

    /// Creates an expression string that corresponds to the `FlatEx` instance. This is
    /// not necessarily the input string. More precisely,
    /// * variable names are forgotten,
    /// * variables are put into curly braces, and
    /// * expressions will be put between parentheses, e.g.,
    /// ```rust
    /// # use std::error::Error;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// #
    /// use exmex::parse_with_default_ops;
    /// let flatex = parse_with_default_ops::<f64>("--sin(z)")?;
    /// assert_eq!(format!("{}", flatex), "-(-(sin({x0})))");
    /// #
    /// #     Ok(())
    /// # }
    /// ```
    ///
    pub fn unparse(&self) -> Result<String, ExParseError> {
        match &self.deepex {
            Some(deepex) => Ok(deepex.unparse()),
            None => Err(ExParseError {
                msg: "unparse impossible, since deep expression optimized away".to_string(),
            }),
        }
    }
    /// Usually, a `FlatEx` instance keeps a nested, deep structure of the expression
    /// that is not necessary for evaluation. This functions removes the deep expression
    /// to reduce memory consumption. [`unparse`](FlatEx::unparse) and
    /// [`Display`](FlatEx::Display) is will stop working after calling this function.
    pub fn clear_deepex(&mut self) {
        self.deepex = None;
    }
}

/// The expression is displayed as a string created by [`unparse`](FlatEx::unparse).
impl<'a, T: Copy + Debug> Display for FlatEx<'a, T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let unparsed = self.unparse();
        match unparsed {
            Err(e) => write!(f, "{}", e.msg),
            Ok(s) => write!(f, "{}", s),
        }
    }
}

/// A deep node can be an expression, a number, or
/// a variable.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum DeepNode<'a, T: Copy + Debug> {
    Expr(DeepEx<'a, T>),
    Num(T),
    /// The contained integer points to the index of the variable in the slice of
    /// variables passed to [`eval`](Expression::eval).
    Var(usize),
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct BinOpsWithReprs<'a, T: Copy> {
    pub reprs: Vec<&'a str>,
    pub ops: BinOpVec<T>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct UnaryOpWithReprs<'a, T: Copy> {
    pub reprs: Vec<&'a str>,
    pub op: UnaryOp<T>,
}
impl<'a, T: Copy> UnaryOpWithReprs<'a, T> {
    pub fn new() -> UnaryOpWithReprs<'a, T> {
        UnaryOpWithReprs {
            reprs: vec![],
            op: UnaryOp::new(),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct OverloadedOps<'a, T: Copy> {
    pub add: Operator<'a, T>,
    pub sub: Operator<'a, T>,
    pub mul: Operator<'a, T>,
    pub div: Operator<'a, T>,
    pub pow: Operator<'a, T>,
}
impl<'a, T: Copy> OverloadedOps<'a, T> {
    pub fn by_repr(&self, repr: &str) -> Operator<'a, T> {
        match repr {
            ADD_REPR => self.add,
            SUB_REPR => self.sub,
            MUL_REPR => self.mul,
            DIV_REPR => self.div,
            POW_REPR => self.pow,
            _ => panic!("{} is not a repr of an overloaded operator", repr),
        }
    }
}
/// A deep expression evaluates co-recursively since its nodes can contain other deep
/// expressions.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct DeepEx<'a, T: Copy + Debug> {
    /// Nodes can be numbers, variables, or other expressions.
    nodes: Vec<DeepNode<'a, T>>,
    /// Binary operators applied to the nodes according to their priority.
    bin_ops: BinOpsWithReprs<'a, T>,
    /// Unary operators are applied to the result of evaluating all nodes with all
    /// binary operators.
    unary_op: UnaryOpWithReprs<'a, T>,
    prio_indices: ExprIdxVec,
    overloaded_ops: Option<OverloadedOps<'a, T>>,
}

impl<'a, T: Copy + Debug> DeepEx<'a, T> {
    /// Evaluates all operators with numbers as operands.
    pub fn compile(&mut self) {
        // change from exression to number if an expression contains only a number
        for node in &mut self.nodes {
            if let DeepNode::Expr(ref e) = node {
                if e.nodes.len() == 1 {
                    match e.nodes[0] {
                        DeepNode::Num(n) => {
                            *node = DeepNode::Num(n);
                        }
                        _ => (),
                    }
                }
            };
        }
        // after changing from expressions to numbers where possible the prios might change
        self.prio_indices = prioritized_indices(&self.bin_ops.ops, &self.nodes);

        let mut num_inds = self.prio_indices.clone();
        let mut used_prio_indices = ExprIdxVec::new();
        for (i, &bin_op_idx) in self.prio_indices.iter().enumerate() {
            let num_idx = num_inds[i];
            let node_1 = &self.nodes[num_idx];
            let node_2 = &self.nodes[num_idx + 1];
            if let (DeepNode::Num(num_1), DeepNode::Num(num_2)) = (node_1, node_2) {
                let bin_op_result = (self.bin_ops.ops[bin_op_idx].apply)(*num_1, *num_2);
                self.nodes[num_idx] = DeepNode::Num(bin_op_result);
                self.nodes.remove(num_idx + 1);
                // reduce indices after removed position
                for num_idx_after in num_inds.iter_mut() {
                    if *num_idx_after > num_idx {
                        *num_idx_after = *num_idx_after - 1;
                    }
                }
                used_prio_indices.push(bin_op_idx);
            } else {
                break;
            }
        }

        self.bin_ops.ops = self
            .bin_ops
            .ops
            .iter()
            .enumerate()
            .filter(|(i, _)| !used_prio_indices.contains(i))
            .map(|x| *x.1)
            .collect();

        if self.nodes.len() == 1 {
            match self.nodes[0] {
                DeepNode::Num(n) => {
                    self.nodes[0] = DeepNode::Num(self.unary_op.op.apply(n));
                    self.unary_op.op.clear();
                    self.unary_op.reprs.clear();
                }
                _ => (),
            }
        }
        self.prio_indices = prioritized_indices(&self.bin_ops.ops, &self.nodes);
    }

    pub fn new(
        nodes: Vec<DeepNode<'a, T>>,
        bin_ops: BinOpsWithReprs<'a, T>,
        unary_op: UnaryOpWithReprs<'a, T>,
    ) -> Result<DeepEx<'a, T>, ExParseError> {
        if nodes.len() != bin_ops.ops.len() + 1 {
            Err(ExParseError {
                msg: "mismatch between number of nodes and binary operators".to_string(),
            })
        } else {
            let indices = prioritized_indices(&bin_ops.ops, &nodes);
            let mut expr = DeepEx {
                nodes: nodes,
                bin_ops: bin_ops,
                unary_op,
                prio_indices: indices,
                overloaded_ops: None,
            };
            expr.compile();
            Ok(expr)
        }
    }

    pub fn unparse(&self) -> String {
        let mut node_strings = self.nodes.iter().map(|n| match n {
            DeepNode::Num(n) => format!("{:?}", n),
            DeepNode::Var(idx) => format!("{{x{}}}", idx),
            DeepNode::Expr(e) => {
                if e.unary_op.op.len() == 0 {
                    format!("({})", e.unparse())
                } else {
                    e.unparse()
                }
            }
        });
        let mut bin_op_strings = self.bin_ops.reprs.iter();
        // a valid expression has at least one node
        let first_node_str = node_strings.next().unwrap();
        let node_with_bin_ops_string = node_strings.fold(first_node_str, |mut res, node_str| {
            let bin_op_str = bin_op_strings.next().unwrap();
            res.push_str(bin_op_str);
            res.push_str(node_str.as_str());
            res
        });
        let unary_op_string = self
            .unary_op
            .reprs
            .iter()
            .fold(String::new(), |mut res, uop_str| {
                res.push_str(uop_str);
                res.push_str("(");
                res
            });
        let closings =
            repeat(")")
                .take(self.unary_op.op.len())
                .fold(String::new(), |mut res, closing| {
                    res.push_str(closing);
                    res
                });
        if self.unary_op.op.len() == 0 {
            node_with_bin_ops_string
        } else {
            format!(
                "{}{}{}",
                unary_op_string, node_with_bin_ops_string, closings
            )
        }
    }

    pub fn set_overloaded_ops(&mut self, overloaded_ops: OverloadedOps<'a, T>) {
        self.overloaded_ops = Some(overloaded_ops);
    }

    /// Careful, if two expressions have different variables this will not work
    /// as expected since variables are identified by the index of their occurrence
    /// instead of their name.
    fn operate_bin(self, other: Self, repr: &str) -> Self {
        if self.overloaded_ops.is_none() {
            panic!("overloaded operators not available");
        }
        let overloaded_ops = self.overloaded_ops.clone();
        let op = overloaded_ops.clone().unwrap().by_repr(repr);

        let ops = smallvec![op.bin_op.unwrap()];
        let mut resex = DeepEx::new(
            vec![DeepNode::Expr(self), DeepNode::Expr(other)],
            BinOpsWithReprs {
                reprs: vec![ADD_REPR],
                ops: ops,
            },
            UnaryOpWithReprs::new(),
        )
        .unwrap();
        resex.overloaded_ops = Some(overloaded_ops.unwrap());
        resex.compile();
        resex
    }

    pub fn pow(self, exponent: Self) -> Self {
        self.operate_bin(exponent, POW_REPR)
    }
}

impl<'a, T: Copy + Debug> Add for DeepEx<'a, T> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        self.operate_bin(other, ADD_REPR)
    }
}

impl<'a, T: Copy + Debug> Sub for DeepEx<'a, T> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        self.operate_bin(other, SUB_REPR)
    }
}

impl<'a, T: Copy + Debug> Mul for DeepEx<'a, T> {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        self.operate_bin(other, MUL_REPR)
    }
}

impl<'a, T: Copy + Debug> Div for DeepEx<'a, T> {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        self.operate_bin(other, DIV_REPR)
    }
}

impl<'a, T: Copy + Debug> Display for DeepEx<'a, T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.unparse())
    }
}

#[cfg(test)]
use crate::{make_default_operators, parse_with_default_ops, util::assert_float_eq_f64};
#[test]
fn test_flat_clear() {
    let mut flatex = parse_with_default_ops::<f64>("x*(2*(2*(2*4*8)))").unwrap();
    assert!(flatex.deepex.is_some());
    flatex.clear_deepex();
    assert!(flatex.deepex.is_none());
    assert_float_eq_f64(flatex.eval(&[1.0]).unwrap(), 2.0 * 2.0 * 2.0 * 4.0 * 8.0);
    assert_eq!(flatex.nodes.len(), 2);
}
#[test]
fn test_flat_compile() {
    let flatex = parse_with_default_ops::<f64>("1*sin(2-0.1)").unwrap();
    assert_float_eq_f64(flatex.eval(&[]).unwrap(), 1.9f64.sin());
    assert_eq!(flatex.nodes.len(), 1);

    let flatex = parse_with_default_ops::<f64>("x*(2*(2*(2*4*8)))").unwrap();
    assert_float_eq_f64(flatex.eval(&[1.0]).unwrap(), 2.0 * 2.0 * 2.0 * 4.0 * 8.0);
    assert_eq!(flatex.nodes.len(), 2);

    let flatex = parse_with_default_ops::<f64>("1*sin(2-0.1) + x").unwrap();
    assert_float_eq_f64(flatex.eval(&[0.0]).unwrap(), 1.9f64.sin());
    assert_eq!(flatex.nodes.len(), 2);
    match flatex.nodes[0].kind {
        FlatNodeKind::Num(n) => assert_float_eq_f64(n, 1.9f64.sin()),
        _ => assert!(false),
    }
    match flatex.nodes[1].kind {
        FlatNodeKind::Var(idx) => assert_eq!(idx, 0),
        _ => assert!(false),
    }

    let flatex = parse_with_default_ops::<f64>("y + 1 - cos(1/(1*sin(2-0.1))-2) + 2 + x").unwrap();
    assert_eq!(flatex.nodes.len(), 3);
    match flatex.nodes[0].kind {
        FlatNodeKind::Var(idx) => assert_eq!(idx, 0),
        _ => assert!(false),
    }
    match flatex.nodes[1].kind {
        FlatNodeKind::Num(_) => (),
        _ => assert!(false),
    }
    match flatex.nodes[2].kind {
        FlatNodeKind::Var(idx) => assert_eq!(idx, 1),
        _ => assert!(false),
    }
}

#[test]
fn test_deep_compile() {
    let ops = make_default_operators();
    let nodes = vec![DeepNode::Num(4.5), DeepNode::Num(0.5), DeepNode::Num(1.4)];
    let bin_ops = BinOpsWithReprs {
        reprs: vec![ops[1].repr, ops[3].repr],
        ops: smallvec![ops[1].bin_op.unwrap(), ops[3].bin_op.unwrap()],
    };
    let unary_op = UnaryOpWithReprs {
        reprs: vec![ops[6].repr],
        op: UnaryOp::from_vec(smallvec![ops[6].unary_op.unwrap()]),
    };
    let deep_ex = DeepEx::new(nodes, bin_ops, unary_op).unwrap();

    let bin_ops = BinOpsWithReprs {
        reprs: vec![ops[1].repr, ops[3].repr],
        ops: smallvec![ops[1].bin_op.unwrap(), ops[3].bin_op.unwrap()],
    };
    let unary_op = UnaryOpWithReprs {
        reprs: vec![ops[6].repr],
        op: UnaryOp::from_vec(smallvec![ops[6].unary_op.unwrap()]),
    };
    let nodes = vec![
        DeepNode::Num(4.5),
        DeepNode::Num(0.5),
        DeepNode::Expr(deep_ex),
    ];
    let deep_ex = DeepEx::new(nodes, bin_ops, unary_op).unwrap();
    assert_eq!(deep_ex.nodes.len(), 1);
    match deep_ex.nodes[0] {
        DeepNode::Num(n) => assert_eq!(deep_ex.unary_op.op.apply(n), n),
        _ => {
            assert!(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        expression::{flatten, DeepEx},
        parse_with_default_ops,
        util::assert_float_eq_f64,
    };

    #[test]
    fn test_operator_overloading() {
        fn from_str<'a>(s: &'a str) -> DeepEx<'a, f64> {
            parse_with_default_ops::<f64>(s).unwrap().deepex.unwrap()
        }
        fn eval<'a>(deepex: &DeepEx<'a, f64>, vars: &[f64], val: f64) {
            assert_float_eq_f64(flatten(deepex.clone()).eval(vars).unwrap(), val);
        }

        let one = from_str("1");
        let two = one.clone() + one.clone();
        eval(&two, &[], 2.0);

        let x_squared = from_str("x^2");
        let x_to_the_4 = x_squared.clone().pow(two.clone());
        eval(&x_to_the_4, &[0.0], 0.0);
        eval(&x_to_the_4, &[1.0], 1.0);
        eval(&x_to_the_4, &[2.0], 16.0);
        eval(&x_to_the_4, &[3.0], 81.0);
        let two_x_squared = two.clone() * x_squared.clone();
        eval(&two_x_squared, &[0.0], 0.0);
        eval(&two_x_squared, &[1.0], 2.0);
        eval(&two_x_squared, &[2.0], 8.0);
        eval(&two_x_squared, &[3.0], 18.0);
        let sqrt = from_str("x").pow(from_str("1") - from_str(".5"));
        eval(&sqrt, &[4.0], 2.0);
        eval(&sqrt, &[25.0], 5.0);
        let some_expr = from_str("x") + from_str("x") * from_str("2") / from_str("x^(.5)");
        eval(&some_expr, &[4.0], 8.0);
    }

    #[test]
    fn test_display() {
        let mut flatex = parse_with_default_ops::<f64>("sin(var)/5").unwrap();
        assert_eq!(format!("{}", flatex), "sin({x0})/5.0");
        flatex.clear_deepex();
        assert_eq!(
            format!("{}", flatex),
            "unparse impossible, since deep expression optimized away"
        );
    }

    #[test]
    fn test_unparse() {
        fn test(text: &str, text_ref: &str) {
            let flatex = parse_with_default_ops::<f64>(text);
            let deepex = flatex.unwrap().deepex.unwrap();

            assert_eq!(deepex.unparse(), text_ref);
            let mut flatex_reparsed = parse_with_default_ops::<f64>(text_ref).unwrap();
            assert_eq!(flatex_reparsed.unparse().unwrap(), text_ref);
            flatex_reparsed.clear_deepex();
            assert!(flatex_reparsed.unparse().is_err());
        }
        let text = "5+x";
        let text_ref = "5.0+{x0}";
        test(text, text_ref);
        let text = "sin(5+var)^(1/{y})+{var}";
        let text_ref = "sin(5.0+{x0})^(1.0/{x1})+{x0}";
        test(text, text_ref);
        let text = "-(5+var)^(1/{y})+{var}";
        let text_ref = "-(5.0+{x0})^(1.0/{x1})+{x0}";
        test(text, text_ref);
        let text = "cos(sin(-(5+var)^(1/{y})))+{var}";
        let text_ref = "cos(sin(-(5.0+{x0})^(1.0/{x1})))+{x0}";
        test(text, text_ref);
        let text = "cos(sin(-5+var^(1/{y})))-{var}";
        let text_ref = "cos(sin(-5.0+{x0}^(1.0/{x1})))-{x0}";
        test(text, text_ref);
        let text = "cos(sin(-z+var*(1/{y})))+{var}";
        let text_ref = "cos(sin(-({x0})+{x1}*(1.0/{x2})))+{x1}";
        test(text, text_ref);
    }
}
