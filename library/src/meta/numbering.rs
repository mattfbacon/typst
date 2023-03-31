use std::str::FromStr;

use ecow::EcoVec;

use crate::prelude::*;
use crate::text::Case;

/// Apply a numbering to a sequence of numbers.
///
/// A numbering defines how a sequence of numbers should be displayed as
/// content. It is defined either through a pattern string or an arbitrary
/// function.
///
/// A numbering pattern consists of counting symbols, for which the actual
/// number is substituted, their prefixes, and one suffix. The prefixes and the
/// suffix are repeated as-is.
///
/// ## Example
/// ```example
/// #numbering("1.1)", 1, 2, 3) \
/// #numbering("1.a.i", 1, 2) \
/// #numbering("I – 1", 12, 2) \
/// #numbering(
///   (..nums) => nums
///     .pos()
///     .map(str)
///     .join(".") + ")",
///   1, 2, 3,
/// )
/// ```
///
/// Display: Numbering
/// Category: meta
/// Returns: any
#[func]
pub fn numbering(
    /// Defines how the numbering works.
    ///
    /// **Counting symbols** are `1`, `a`, `A`, `i`, `I` and `*`. They are
    /// replaced by the number in the sequence, in the given case.
    ///
    /// The `*` character means that symbols should be used to count, in the
    /// order of `*`, `†`, `‡`, `§`, `¶`, and `‖`. If there are more than six
    /// items, the number is represented using multiple symbols.
    ///
    /// **Suffixes** are all characters after the last counting symbol. They are
    /// repeated as-is at the end of any rendered number.
    ///
    /// **Prefixes** are all characters that are neither counting symbols nor
    /// suffixes. They are repeated as-is at in front of their rendered
    /// equivalent of their counting symbol.
    ///
    /// This parameter can also be an arbitrary function that gets each number as
    /// an individual argument. When given a function, the `numbering` function
    /// just forwards the arguments to that function. While this is not
    /// particularly useful in itself, it means that you can just give arbitrary
    /// numberings to the `numbering` function without caring whether they are
    /// defined as a pattern or function.
    numbering: Numbering,
    /// The numbers to apply the numbering to. Must be positive.
    ///
    /// If `numbering` is a pattern and more numbers than counting symbols are
    /// given, the last counting symbol with its prefix is repeated.
    #[variadic]
    numbers: Vec<usize>,
) -> Value {
    numbering.apply_vm(vm, &numbers)?
}

/// How to number a sequence of things.
#[derive(Debug, Clone, PartialEq, Hash)]
pub enum Numbering {
    /// A pattern with prefix, numbering, lower / upper case and suffix.
    Pattern(NumberingPattern),
    /// A closure mapping from an item's number to content.
    Func(Func),
}

impl Numbering {
    /// Apply the pattern to the given numbers.
    ///
    /// # Errors
    ///
    /// If a function-based numbering fails to evaluate.
    #[inline]
    pub fn apply_vm(&self, vm: &mut Vm<'_>, numbers: &[usize]) -> SourceResult<Value> {
        Ok(match self {
            Self::Pattern(pattern) => Value::Str(pattern.apply(numbers).into()),
            Self::Func(func) => {
                let args =
                    Args::new(func.span(), numbers.iter().map(|&n| Value::Int(n as i64)));
                func.call_vm(vm, args)?
            }
        })
    }

    /// Apply the pattern to the given numbers.
    ///
    /// # Errors
    ///
    /// If a function-based numbering fails to evaluate.
    #[inline]
    pub fn apply_vt(&self, vt: &mut Vt<'_>, numbers: &[usize]) -> SourceResult<Value> {
        Ok(match self {
            Self::Pattern(pattern) => Value::Str(pattern.apply(numbers).into()),
            Self::Func(func) => {
                func.call_vt(vt, numbers.iter().map(|&n| Value::Int(n as i64)))?
            }
        })
    }

    /// Trim the prefix suffix if this is a pattern.
    #[inline]
    #[must_use]
    pub fn trimmed(mut self) -> Self {
        if let Self::Pattern(pattern) = &mut self {
            pattern.trimmed = true;
        }
        self
    }
}

impl From<NumberingPattern> for Numbering {
    #[inline]
    fn from(pattern: NumberingPattern) -> Self {
        Self::Pattern(pattern)
    }
}

cast_from_value! {
    Numbering,
    v: NumberingPattern => Self::Pattern(v),
    v: Func => Self::Func(v),
}

cast_to_value! {
    v: Numbering => match v {
        Numbering::Pattern(pattern) => pattern.into(),
        Numbering::Func(func) => func.into(),
    }
}

/// How to turn a number into text.
///
/// A pattern consists of a prefix, followed by one of `1`, `a`, `A`, `i`, `I`
/// or `*`, and then a suffix.
///
/// Examples of valid patterns:
/// - `1)`
/// - `a.`
/// - `(I)`
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct NumberingPattern {
    pieces: EcoVec<(EcoString, NumberingKind, Case)>,
    suffix: EcoString,
    trimmed: bool,
}

impl NumberingPattern {
    /// Apply the pattern to the given number.
    #[must_use]
    pub fn apply(&self, numbers: &[usize]) -> EcoString {
        let mut fmt = EcoString::new();
        let mut numbers = numbers.iter();

        for (i, ((prefix, kind, case), &number)) in
            self.pieces.iter().zip(&mut numbers).enumerate()
        {
            if i > 0 || !self.trimmed {
                fmt.push_str(prefix);
            }
            fmt.push_str(&kind.apply(number, *case));
        }

        for ((prefix, kind, case), &number) in
            self.pieces.last().into_iter().cycle().zip(numbers)
        {
            if prefix.is_empty() {
                fmt.push_str(&self.suffix);
            } else {
                fmt.push_str(prefix);
            }
            fmt.push_str(&kind.apply(number, *case));
        }

        if !self.trimmed {
            fmt.push_str(&self.suffix);
        }

        fmt
    }

    /// Apply only the k-th segment of the pattern to a number.
    #[must_use]
    pub fn apply_kth(&self, k: usize, number: usize) -> EcoString {
        let mut fmt = EcoString::new();
        if let Some((prefix, _, _)) = self.pieces.first() {
            fmt.push_str(prefix);
        }
        if let Some((_, kind, case)) = self
            .pieces
            .iter()
            .chain(self.pieces.last().into_iter().cycle())
            .nth(k)
        {
            fmt.push_str(&kind.apply(number, *case));
        }
        fmt.push_str(&self.suffix);
        fmt
    }

    /// How many counting symbols this pattern has.
    #[inline]
    #[must_use]
    pub fn pieces(&self) -> usize {
        self.pieces.len()
    }
}

impl FromStr for NumberingPattern {
    type Err = &'static str;

    fn from_str(pattern: &str) -> Result<Self, Self::Err> {
        let mut pieces = EcoVec::new();
        let mut handled = 0;

        for (i, c) in pattern.char_indices() {
            let Some(kind) = NumberingKind::from_char(c.to_ascii_lowercase()) else {
                continue;
            };

            let prefix = pattern[handled..i].into();
            let case = if c.is_uppercase() { Case::Upper } else { Case::Lower };
            pieces.push((prefix, kind, case));
            handled = i + 1;
        }

        let suffix = pattern[handled..].into();
        if pieces.is_empty() {
            Err("invalid numbering pattern")?;
        }

        Ok(Self { pieces, suffix, trimmed: false })
    }
}

cast_from_value! {
    NumberingPattern,
    v: Str => v.parse()?,
}

cast_to_value! {
    v: NumberingPattern => {
        let mut pat = EcoString::new();
        for (prefix, kind, case) in &v.pieces {
            pat.push_str(prefix);
            let mut c = kind.to_char();
            if *case == Case::Upper {
                c = c.to_ascii_uppercase();
            }
            pat.push(c);
        }
        pat.push_str(&v.suffix);
        pat.into()
    }
}

/// Different kinds of numberings.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum NumberingKind {
    Arabic,
    Letter,
    Roman,
    Symbol,
}

impl NumberingKind {
    /// Create a numbering kind from a lowercase character.
    #[inline]
    #[must_use]
    pub fn from_char(c: char) -> Option<Self> {
        Some(match c {
            '1' => NumberingKind::Arabic,
            'a' => NumberingKind::Letter,
            'i' => NumberingKind::Roman,
            '*' => NumberingKind::Symbol,
            _ => return None,
        })
    }

    /// The lowercase character for this numbering kind.
    #[inline]
    #[must_use]
    pub fn to_char(self) -> char {
        match self {
            Self::Arabic => '1',
            Self::Letter => 'a',
            Self::Roman => 'i',
            Self::Symbol => '*',
        }
    }

    /// Apply the numbering to the given number.
    #[must_use]
    pub fn apply(self, mut numbering: usize, case: Case) -> EcoString {
        match self {
            Self::Arabic => {
                eco_format!("{numbering}")
            }
            Self::Letter => {
                if numbering == 0 {
                    return '-'.into();
                }

                numbering -= 1;

                let mut letters = vec![];
                loop {
                    let ch = b'a' + u8::try_from(numbering % 26).unwrap();
                    letters.push(match case {
                        Case::Lower => ch,
                        Case::Upper => ch.to_ascii_uppercase(),
                    });
                    numbering /= 26;
                    if numbering == 0 {
                        break;
                    }
                }

                letters.reverse();
                String::from_utf8(letters).unwrap().into()
            }
            Self::Roman => {
                // Adapted from Yann Villessuzanne's roman.rs under the
                // Unlicense, at https://github.com/linfir/roman.rs/
                const ROMAN: &[(&str, usize)] = &[
                    ("M̅", 1_000_000),
                    ("D̅", 500_000),
                    ("C̅", 100_000),
                    ("L̅", 50_000),
                    ("X̅", 10_000),
                    ("V̅", 5_000),
                    ("I̅V̅", 4_000),
                    ("M", 1_000),
                    ("CM", 900),
                    ("D", 500),
                    ("CD", 400),
                    ("C", 100),
                    ("XC", 90),
                    ("L", 50),
                    ("XL", 40),
                    ("X", 10),
                    ("IX", 9),
                    ("V", 5),
                    ("IV", 4),
                    ("I", 1),
                ];

                if numbering == 0 {
                    return 'N'.into();
                }

                let mut fmt = EcoString::new();
                for &(name, value) in ROMAN {
                    while numbering >= value {
                        numbering -= value;
                        for ch in name.chars() {
                            match case {
                                Case::Lower => fmt.extend(ch.to_lowercase()),
                                Case::Upper => fmt.push(ch),
                            }
                        }
                    }
                }

                fmt
            }
            Self::Symbol => {
                const SYMBOLS: &[char] = &['*', '†', '‡', '§', '¶', '‖'];

                if numbering == 0 {
                    return '-'.into();
                }

                let symbol = SYMBOLS[(numbering - 1) % SYMBOLS.len()];
                let amount = ((numbering - 1) / SYMBOLS.len()) + 1;
                std::iter::repeat(symbol).take(amount).collect()
            }
        }
    }
}
