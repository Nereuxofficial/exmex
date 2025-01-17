#![doc(html_root_url = "https://docs.rs/exmex/0.8.4")]
//! Exmex is a fast, simple, and **ex**tendable **m**athematical **ex**pression evaluator.  
//! ```rust
//! # use std::error::Error;
//! # fn main() -> Result<(), Box<dyn Error>> {
//! #
//! use exmex::eval_str;
//! assert!((eval_str("1.5 * ((cos(0) + 23.0) / 2.0)")? - 18.0).abs() < 1e-12);
//! #
//! #     Ok(())
//! # }
//! ```
//! For floats, we have a list of predifined operators containing
//! `^`, `*`, `/`, `+`, `-`, `sin`, `cos`, `tan`, `exp`, `log`, and `log2`. The full list is
//! defined in [`make_default_operators`](make_default_operators).
//!
//! ## Variables
//! For variables we can use strings that are not in the list of operators as shown in the following expression.
//! Additionally, variables should consist only of letters, numbers, and underscores. More precisely, they need to fit the
//! regular expression
//! ```r"^[a-zA-Z_]+[a-zA-Z_0-9]*"```.
//! Variables' values are passed as slices to [`eval`](FlatEx::eval).
//! ```rust
//! # use std::error::Error;
//! # fn main() -> Result<(), Box<dyn Error>> {
//! #
//! use exmex::{make_default_operators, parse};
//! let to_be_parsed = "log(z) + 2* (-z^2 + sin(4*y))";
//! let expr = parse::<f64>(to_be_parsed, &make_default_operators::<f64>())?;
//! assert!((expr.eval(&[3.7, 2.5])? - 14.992794866624788 as f64).abs() < 1e-12);
//! #
//! #     Ok(())
//! # }
//! ```
//! The `n`-th number in the slice corresponds to the `n`-th variable. Thereby, the
//! alphatical order of the variables is relevant. In this example, we have `y=3.7` and `z=2.5`.
//! If variables are between curly brackets, they can have arbitrary names, e.g.,
//! `{456/549*(}`, `{x}`, and confusingly even `{x+y}` are valid variable names as shown in the following.
//! ```rust
//! # use std::error::Error;
//! # fn main() -> Result<(), Box<dyn Error>> {
//! #
//! use exmex::{make_default_operators, parse};
//! let x = 2.1f64;
//! let y = 0.1f64;
//! let to_be_parsed = "log({x+y})";  // {x+y} is the name of one(!) variable 😕.
//! let expr = parse::<f64>(to_be_parsed, &make_default_operators::<f64>())?;
//! assert!((expr.eval(&[x+y])? - 2.2f64.ln()).abs() < 1e-12);
//! #
//! #     Ok(())
//! # }
//! ```
//! ## Extendability
//! Library users can define their own set of operators as shown in the following.
//! ```rust
//! # use std::error::Error;
//! # fn main() -> Result<(), Box<dyn Error>> {
//! #
//! use exmex::{parse, BinOp, Operator};
//! let ops = [
//!     Operator {
//!         repr: "%",
//!         bin_op: Some(BinOp{ apply: |a: i32, b: i32| a % b, prio: 1 }),
//!         unary_op: None,
//!     },
//!     Operator {
//!         repr: "/",
//!         bin_op: Some(BinOp{ apply: |a: i32, b: i32| a / b, prio: 1 }),
//!         unary_op: None,
//!     },
//! ];
//! let to_be_parsed = "19 % 5 / 2 / a";
//! let expr = parse::<i32>(to_be_parsed, &ops)?;
//! assert_eq!(expr.eval(&[1])?, 2);
//! #
//! #     Ok(())
//! # }
//! ```
//!
//! ### Operators
//!
//! Operators are instances of the struct
//! [`Operator`](Operator) that has its representation in the field
//! [`repr`](Operator::repr), a binary and a unary operator of
//! type [`Option<BinOp<T>>`](Operator::bin_op) and
//! [`Option<fn(T) -> T>`](Operator::unary_op), respectively, as
//! members. [`BinOp`](BinOp)
//! contains in addition to the function pointer [`apply`](BinOp::apply) of type `fn(T, T) -> T` an
//! integer [`prio`](BinOp::prio). Operators
//! can be both, binary and unary. See, e.g.,  `-` defined in the list of default
//! operators. Note that we expect a unary operator to be always on the left of a
//! number.
//!
//! ### Data Types of Numbers
//!
//! You can use any type that implements [`Copy`](core::marker::Copy) and
//! [`FromStr`](std::str::FromStr). In case the representation of your data type in the
//! string does not match the number regex `r"\.?[0-9]+(\.[0-9]+)?"`, you have to pass a
//! suitable regex and use the function
//! [`parse_with_number_pattern`](parse_with_number_pattern) instead of
//! [`parse`](parse). Here is an example for `bool`.
//! ```rust
//! # use std::error::Error;
//! # fn main() -> Result<(), Box<dyn Error>> {
//! #
//! use exmex::{parse_with_number_pattern, BinOp, Operator};
//! let ops = [
//!     Operator {
//!         repr: "&&",
//!         bin_op: Some(BinOp{ apply: |a: bool, b: bool| a && b, prio: 1 }),
//!         unary_op: None,
//!     },
//!     Operator {
//!         repr: "||",
//!         bin_op: Some(BinOp{ apply: |a: bool, b: bool| a || b, prio: 1 }),
//!         unary_op: None,
//!     },
//!     Operator {
//!         repr: "!",
//!         bin_op: None,
//!         unary_op: Some(|a: bool| !a),
//!     },
//! ];
//! let to_be_parsed = "!(true && false) || (!false || (true && false))";
//! let expr = parse_with_number_pattern::<bool>(to_be_parsed, &ops, "true|false")?;
//! assert_eq!(expr.eval(&[])?, true);
//! #
//! #     Ok(())
//! # }
//! ```
//!
//! ## Priorities and Parentheses
//! In Exmex-land, unary operators always have higher priority than binary operators, e.g.,
//! `-2^2=4` instead of `-2^2=-4`. Moreover, we are not too strict regarding parentheses.
//! For instance
//! ```rust
//! # use std::error::Error;
//! # fn main() -> Result<(), Box<dyn Error>> {
//! #
//! use exmex::eval_str;
//! assert_eq!(eval_str("---1")?, -1.0);
//! #
//! #     Ok(())
//! # }
//! ```
//! If you want to be on the safe side, we suggest using parentheses.
//!
//! ## Partial Derivatives
//!
//! For default operators, expressions can be transformed into their partial derivatives
//! again represented by expressions.
//! ```rust
//! # use std::error::Error;
//! # fn main() -> Result<(), Box<dyn Error>> {
//! #
//! use exmex::{parse_with_default_ops};
//!
//! let expr = parse_with_default_ops::<f64>("x^2 + y^2")?;
//! let d_x = expr.clone().partial(0)?;
//! let d_y = expr.partial(1)?;
//! assert!((d_x.eval(&[3.0, 2.0])? - 6.0).abs() < 1e-12);
//! assert!((d_y.eval(&[3.0, 2.0])? - 4.0).abs() < 1e-12);
//! #
//! #     Ok(())
//! # }
//! ```
//!
//! ## Display
//!
//! An instance of [`FlatEx`](FlatEx) can be displayed as string. Note that this
//! [`unparse`](FlatEx::unparse)d string does not necessarily coincide with the original
//! string, since curly brackets are added.
//!
//! ```rust
//! # use std::error::Error;
//! # fn main() -> Result<(), Box<dyn Error>> {
//! #
//! use exmex::parse_with_default_ops;
//! let flatex = parse_with_default_ops::<f64>("-sin(z)/cos(mother_of_names)")?;
//! assert_eq!(format!("{}", flatex), "-(sin({z}))/cos({mother_of_names})");
//! #
//! #     Ok(())
//! # }
//! ```
//!
//! ## Unicode
//! Unicode input strings are currently not supported 😕 but might be added in the
//! future 😀.
//!

mod definitions;
mod expression;
mod operators;
mod parser;
mod util;

use std::{fmt::Debug, str::FromStr};

pub use expression::flat::FlatEx;
use expression::{deep::DeepEx, flat};

use num::Float;
pub use parser::ExParseError;

pub use operators::{make_default_operators, BinOp, Operator};

/// Parses a string, evaluates a string, and returns the resulting number.
///
/// # Errrors
///
/// In case the parsing went wrong, e.g., due to an invalid input string, an
/// [`ExParseError`](ExParseError) is returned.
///
pub fn eval_str(text: &str) -> Result<f64, ExParseError> {
    let flatex = parse_with_default_ops(text)?;
    flatex.eval(&[])
}

/// Parses a string and a vector of operators into an expression that can be evaluated.
///
/// # Errors
///
/// An error is returned in case [`parse_with_number_pattern`](parse_with_number_pattern)
/// returns one.
pub fn parse<'a, T>(text: &'a str, ops: &[Operator<'a, T>]) -> Result<FlatEx<'a, T>, ExParseError>
where
    <T as std::str::FromStr>::Err: Debug,
    T: Copy + FromStr + Debug,
{
    let deepex = DeepEx::from_ops(text, ops)?;
    Ok(flat::flatten(deepex))
}

/// Parses a string and a vector of operators and a regex pattern that defines the looks
/// of a number into an expression that can be evaluated.
///
/// # Errors
///
/// An [`ExParseError`](ExParseError) is returned, if
///
//
// from apply_regexes
//
/// * the argument `number_regex_pattern` cannot be compiled,
/// * the argument `text` contained a character that did not match any regex (e.g.,
///   if there is a `Δ` in `text` but no [operator](Operator) with
///   [`repr`](Operator::repr) equal to `Δ` is given),
//
// from check_preconditions
//
/// * the to-be-parsed string is empty,
/// * a number or variable is next to another one, e.g., `2 {x}`,
/// * wlog a number or variable is on the right of a closing parenthesis, e.g., `)5`,
/// * a binary operator is next to another binary operator, e.g., `2*/4`,
/// * wlog a closing parenthesis is next to an opening one, e.g., `)(` or `()`,
/// * too many closing parentheses at some position, e.g., `(4+6) - 5)*2`,
/// * the last element is an operator, e.g., `1+`,
/// * the number of opening and closing parenthesis do not match, e.g., `((4-2)`,
//
// from make_expression
//
/// * in `parsed_tokens` a closing parentheses is directly following an operator, e.g., `+)`, or
/// * a unary operator is followed directly by a binary operator, e.g., `sin*`.
///
pub fn parse_with_number_pattern<'a, T>(
    text: &'a str,
    ops: &[Operator<'a, T>],
    number_regex_pattern: &str,
) -> Result<FlatEx<'a, T>, ExParseError>
where
    <T as std::str::FromStr>::Err: Debug,
    T: Copy + FromStr + Debug,
{
    let deepex = DeepEx::from_pattern(text, ops, number_regex_pattern)?;
    Ok(flat::flatten(deepex))
}

/// Parses a string into an expression that can be evaluated using default operators.
///
/// # Errors
///
/// An error is returned in case [`parse`](parse)
/// returns one.
pub fn parse_with_default_ops<'a, T>(text: &'a str) -> Result<FlatEx<'a, T>, ExParseError>
where
    <T as std::str::FromStr>::Err: Debug,
    T: Float + FromStr + Debug,
{
    Ok(flat::flatten(DeepEx::from_str(text)?))
}

#[cfg(test)]
mod tests {

    use std::iter::once;

    use crate::{
        eval_str,
        operators::{make_default_operators, BinOp, Operator},
        parse, parse_with_default_ops,
        util::{assert_float_eq_f32, assert_float_eq_f64},
        ExParseError,
    };

    #[test]
    fn test_readme() {
        fn readme() -> Result<f64, ExParseError> {
            let result = eval_str("sin(73)")?;
            assert_float_eq_f64(result, 73f64.sin());
            let expr = parse_with_default_ops::<f64>("2*x^3-4/z")?;
            let value = expr.eval(&[5.3, 0.5])?;
            assert_float_eq_f64(value, 289.75399999999996);
            Ok(value)
        }
        fn readme_int() -> Result<u32, ExParseError> {
            let ops = vec![
                Operator {
                    repr: "|",
                    bin_op: Some(BinOp {
                        apply: |a: u32, b: u32| a | b,
                        prio: 0,
                    }),
                    unary_op: None,
                },
                Operator {
                    repr: "!",
                    bin_op: None,
                    unary_op: Some(|a: u32| !a),
                },
            ];
            let expr = parse::<u32>("!(a|b)", &ops)?;
            let result = expr.eval(&[0, 1])?;
            assert_eq!(result, u32::MAX - 1);
            Ok(result)
        }
        assert!(!readme().is_err());
        assert!(!readme_int().is_err());
    }
    #[test]
    fn test_variables_curly() {
        let sut = "5*{x} + 4*log2(log(1.5-{gamma}))*({x}*-(tan(cos(sin(652.2-{gamma}))))) + 3*{x}";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[1.0, 0.0]).unwrap(), 11.429314405093656);
        let sut = "2*(4*{x} + y^2)";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[2.0, 3.0]).unwrap(), 34.0);

        let sut = "sin({myvwmlf4i58eo;w/-😕+sin(a)r_25})";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[1.5707963267948966]).unwrap(), 1.0);

        let sut = "((sin({myvar_25})))";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[1.5707963267948966]).unwrap(), 1.0);
    }
    #[test]
    fn test_variables() {
        let sut = "sin({x})+(((cos({y})^(sin({z})))*log(cos({y})))*cos({z}))";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        let reference =
            |x: f64, y: f64, z: f64| x.sin() + y.cos().powf(z.sin()) * y.cos().ln() * z.cos();

        assert_float_eq_f64(
            expr.eval(&[-0.18961918881278095, -6.383306547710852, 3.1742139703464503])
                .unwrap(),
            reference(-0.18961918881278095, -6.383306547710852, 3.1742139703464503),
        );

        let sut = "sin(sin(x - 1 / sin(y * 5)) + (5.0 - 1/z))";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        let reference =
            |x: f64, y: f64, z: f64| ((x - 1.0 / (y * 5.0).sin()).sin() + (5.0 - 1.0 / z)).sin();
        assert_float_eq_f64(
            expr.eval(&[1.0, 2.0, 4.0]).unwrap(),
            reference(1.0, 2.0, 4.0),
        );

        let sut = "0.02*sin(-(3*(2*(5.0 - 1/z))))";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        let reference = |z: f64| 0.02 * (-(3.0 * (2.0 * (5.0 - 1.0 / z)))).sin();
        assert_float_eq_f64(expr.eval(&[4.0]).unwrap(), reference(4.0));

        let sut = "y + 1 + 0.5 * x";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[3.0, 1.0]).unwrap(), 3.5);

        let sut = " -(-(1+x))";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[1.0]).unwrap(), 2.0);

        let sut = " sin(cos(-3.14159265358979*x))";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[1.0]).unwrap(), -0.841470984807896);

        let sut = "5*sin(x * (4-y^(2-x) * 3 * cos(x-2*(y-1/(y-2*1/cos(sin(x*y))))))*x)";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[1.5, 0.2532]).unwrap(), -3.1164569260604176);

        let sut = "5*x + 4*y + 3*x";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[1.0, 0.0]).unwrap(), 8.0);

        let sut = "5*x + 4*y";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[0.0, 1.0]).unwrap(), 4.0);

        let sut = "5*x + 4*y + x^2";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[2.5, 3.7]).unwrap(), 33.55);
        assert_float_eq_f64(expr.eval(&[12.0, 9.3]).unwrap(), 241.2);

        let sut = "2*(4*x + y^2)";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[2.0, 3.0]).unwrap(), 34.0);

        let sut = "sin(myvar_25)";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[1.5707963267948966]).unwrap(), 1.0);

        let sut = "((sin(myvar_25)))";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[1.5707963267948966]).unwrap(), 1.0);

        let sut = "(0 * myvar_25 + cos(x))";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(
            expr.eval(&[1.5707963267948966, 3.141592653589793]).unwrap(),
            -1.0,
        );

        let sut = "(-x^2)";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[1.0]).unwrap(), 1.0);

        let sut = "log(x) + 2* (-x^2 + sin(4*y))";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[2.5, 3.7]).unwrap(), 14.992794866624788);

        let sut = "-sqrt(x)/(tanh(5-x)*2) + floor(2.4)* 1/asin(-x^2 + sin(4*sinh(y)))";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(
            expr.eval(&[2.5, 3.7]).unwrap(),
            -(2.5f64.sqrt()) / (2.5f64.tanh() * 2.0)
                + 2.0 / ((3.7f64.sinh() * 4.0).sin() + 2.5 * 2.5).asin(),
        );

        let sut = "asin(sin(x)) + acos(cos(x)) + atan(tan(x))";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[0.5]).unwrap(), 1.5);

        let sut = "sqrt(alpha^ceil(centauri))";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[2.0, 3.1]).unwrap(), 4.0);

        let sut = "trunc(x) + fract(x)";
        let expr = parse_with_default_ops::<f64>(sut).unwrap();
        assert_float_eq_f64(expr.eval(&[23422.52345]).unwrap(), 23422.52345);
    }

    #[test]
    fn test_custom_ops_invert() {
        let ops = vec![
            Operator {
                repr: "invert",
                bin_op: None,
                unary_op: Some(|a: f32| 1.0 / a),
            },
            Operator {
                repr: "sqrt",
                bin_op: None,
                unary_op: Some(|a: f32| a.sqrt()),
            },
        ];
        let expr = parse("sqrt(invert(a))", &ops).unwrap();
        assert_float_eq_f32(expr.eval(&[0.25]).unwrap(), 2.0);
    }

    #[test]
    fn test_custom_ops() {
        let custom_ops = vec![
            Operator {
                repr: "**",
                bin_op: Some(BinOp {
                    apply: |a: f32, b| a.powf(b),
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
                repr: "invert",
                bin_op: None,
                unary_op: Some(|a: f32| 1.0 / a),
            },
        ];
        let expr = parse("2**2*invert(3)", &custom_ops).unwrap();
        let val = expr.eval(&[]).unwrap();
        assert_float_eq_f32(val, 4.0 / 3.0);

        let zero_mapper = Operator {
            repr: "zer0",
            bin_op: Some(BinOp {
                apply: |_: f32, _| 0.0,
                prio: 2,
            }),
            unary_op: Some(|_| 0.0),
        };
        let extended_operators = make_default_operators::<f32>()
            .iter()
            .cloned()
            .chain(once(zero_mapper))
            .collect::<Vec<_>>();
        let expr = parse("2^2*1/(berti) + zer0(4)", &extended_operators).unwrap();
        let val = expr.eval(&[4.0]).unwrap();
        assert_float_eq_f32(val, 1.0);
    }

    #[test]
    fn test_eval() {
        assert_float_eq_f64(eval_str("2*3^2").unwrap(), 18.0);
        assert_float_eq_f64(eval_str("-3^2").unwrap(), 9.0);
        assert_float_eq_f64(eval_str("11.3").unwrap(), 11.3);
        assert_float_eq_f64(eval_str("+11.3").unwrap(), 11.3);
        assert_float_eq_f64(eval_str("-11.3").unwrap(), -11.3);
        assert_float_eq_f64(eval_str("(-11.3)").unwrap(), -11.3);
        assert_float_eq_f64(eval_str("11.3+0.7").unwrap(), 12.0);
        assert_float_eq_f64(eval_str("31.3+0.7*2").unwrap(), 32.7);
        assert_float_eq_f64(eval_str("1.3+0.7*2-1").unwrap(), 1.7);
        assert_float_eq_f64(eval_str("1.3+0.7*2-1/10").unwrap(), 2.6);
        assert_float_eq_f64(eval_str("(1.3+0.7)*2-1/10").unwrap(), 3.9);
        assert_float_eq_f64(eval_str("1.3+(0.7*2)-1/10").unwrap(), 2.6);
        assert_float_eq_f64(eval_str("1.3+0.7*(2-1)/10").unwrap(), 1.37);
        assert_float_eq_f64(eval_str("1.3+0.7*(2-1/10)").unwrap(), 2.63);
        assert_float_eq_f64(eval_str("-1*(1.3+0.7*(2-1/10))").unwrap(), -2.63);
        assert_float_eq_f64(eval_str("-1*(1.3+(-0.7)*(2-1/10))").unwrap(), 0.03);
        assert_float_eq_f64(eval_str("-1*((1.3+0.7)*(2-1/10))").unwrap(), -3.8);
        assert_float_eq_f64(eval_str("sin(3.14159265358979)").unwrap(), 0.0);
        assert_float_eq_f64(eval_str("0-sin(3.14159265358979 / 2)").unwrap(), -1.0);
        assert_float_eq_f64(eval_str("-sin(3.14159265358979 / 2)").unwrap(), -1.0);
        assert_float_eq_f64(eval_str("3-(-1+sin(1.5707963267948966)*2)").unwrap(), 2.0);
        assert_float_eq_f64(
            eval_str("3-(-1+sin(cos(-3.14159265358979))*2)").unwrap(),
            5.6829419696157935,
        );
        assert_float_eq_f64(
            eval_str("-(-1+((-3.14159265358979)/5)*2)").unwrap(),
            2.256637061435916,
        );
        assert_float_eq_f64(eval_str("((2-4)/5)*2").unwrap(), -0.8);
        assert_float_eq_f64(
            eval_str("-(-1+(sin(-3.14159265358979)/5)*2)").unwrap(),
            1.0,
        );
        assert_float_eq_f64(
            eval_str("-(-1+sin(cos(-3.14159265358979)/5)*2)").unwrap(),
            1.3973386615901224,
        );
        assert_float_eq_f64(eval_str("-cos(3.14159265358979)").unwrap(), 1.0);
        assert_float_eq_f64(
            eval_str("1+sin(-cos(-3.14159265358979))").unwrap(),
            1.8414709848078965,
        );
        assert_float_eq_f64(
            eval_str("-1+sin(-cos(-3.14159265358979))").unwrap(),
            -0.1585290151921035,
        );
        assert_float_eq_f64(
            eval_str("-(-1+sin(-cos(-3.14159265358979)/5)*2)").unwrap(),
            0.6026613384098776,
        );
        assert_float_eq_f64(eval_str("sin(-(2))*2").unwrap(), -1.8185948536513634);
        assert_float_eq_f64(eval_str("sin(sin(2))*2").unwrap(), 1.5781446871457767);
        assert_float_eq_f64(eval_str("sin(-(sin(2)))*2").unwrap(), -1.5781446871457767);
        assert_float_eq_f64(eval_str("-sin(2)*2").unwrap(), -1.8185948536513634);
        assert_float_eq_f64(eval_str("sin(-sin(2))*2").unwrap(), -1.5781446871457767);
        assert_float_eq_f64(eval_str("sin(-sin(2)^2)*2").unwrap(), 1.4715655294841483);
        assert_float_eq_f64(
            eval_str("sin(-sin(2)*-sin(2))*2").unwrap(),
            1.4715655294841483,
        );
        assert_float_eq_f64(eval_str("--(1)").unwrap(), 1.0);
        assert_float_eq_f64(eval_str("--1").unwrap(), 1.0);
        assert_float_eq_f64(eval_str("----1").unwrap(), 1.0);
        assert_float_eq_f64(eval_str("---1").unwrap(), -1.0);
        assert_float_eq_f64(eval_str("3-(4-2/3+(1-2*2))").unwrap(), 2.666666666666666);
        assert_float_eq_f64(
            eval_str("log(log(2))*tan(2)+exp(1.5)").unwrap(),
            5.2825344122094045,
        );
        assert_float_eq_f64(
            eval_str("log(log2(2))*tan(2)+exp(1.5)").unwrap(),
            4.4816890703380645,
        );
        assert_float_eq_f64(eval_str("log2(2)").unwrap(), 1.0);
        assert_float_eq_f64(eval_str("2^log2(2)").unwrap(), 2.0);
        assert_float_eq_f64(eval_str("2^(cos(0)+2)").unwrap(), 8.0);
        assert_float_eq_f64(eval_str("2^cos(0)+2").unwrap(), 4.0);
    }

    #[test]
    fn test_error_handling() {
        assert!(eval_str("").is_err());
        assert!(eval_str("5+5-(").is_err());
        assert!(eval_str(")2*(5+5)*3-2)*2").is_err());
        assert!(eval_str("2*(5+5))").is_err());
    }
}
