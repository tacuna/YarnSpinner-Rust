//! Post-processes the compiled program to generate hub nodes for every node group.
//!
//! Each node group `"GroupTitle"` becomes:
//!  - Several member nodes with unique names (`"GroupTitle.0"`, `"GroupTitle.1"`, …)
//!    — already compiled with those unique names by `generate_code`.
//!  - A synthetic hub node named `"GroupTitle"` that evaluates `when:` conditions,
//!    runs `AddSaliencyCandidate` for each member, selects the best via
//!    `SelectSaliencyCandidate`, and `DetourToNode`s into the winner.

use crate::prelude::*;
use std::collections::BTreeMap;
use yarnspinner_core::prelude::*;

pub(crate) fn compile_node_groups(mut state: CompilationIntermediate) -> CompilationIntermediate {
    if state.node_groups.is_empty() {
        return state;
    }

    let Ok(compilation) = state.result.as_mut().unwrap().as_mut() else {
        return state;
    };

    if compilation.program.is_none() {
        return state;
    }

    // Clone the data we need (can't hold immutable refs while mutating compilation).
    let groups: Vec<_> = state
        .node_groups
        .iter()
        .map(|(title, members)| {
            let tracking_var = state
                .tracking_nodes
                .contains(title)
                .then(|| Library::generate_unique_visited_variable_for_node(title));
            (title.clone(), members.clone(), tracking_var)
        })
        .collect();

    for (group_title, members, tracking_var) in groups {
        let hub_node = generate_hub_node(&group_title, &members, tracking_var.as_deref());
        // Insert the synthesized hub node into the program.
        compilation.program.as_mut().unwrap().nodes.insert(group_title.clone(), hub_node);
        // Insert a DebugInfo entry marking this as an implicit (compiler-generated) node.
        compilation.debug_info.nodes.push(DebugInfo {
            node_name: group_title,
            is_implicit: true,
            ..Default::default()
        });
    }

    state
}

// ---------------------------------------------------------------------------
// Hub-node builder
// ---------------------------------------------------------------------------

fn generate_hub_node(group_title: &str, members: &[crate::compiler::run_compilation::NodeGroupMember], tracking_var: Option<&str>) -> Node {
    let mut instrs: Vec<Instruction> = Vec::new();
    let mut labels: BTreeMap<String, i32> = BTreeMap::new();

    // Every compiled node must have a "Start" label at index 0.
    labels.insert("Start".to_owned(), 0);

    // Optional visit-tracking code: $Yarn.Internal.Visiting.<hub> += 1
    if let Some(var) = tracking_var {
        push_instr(&mut instrs, push_var(var));
        push_instr(&mut instrs, push_float(1.0));
        push_instr(&mut instrs, push_float(2.0)); // arg count for Number.Add
        push_instr(&mut instrs, call_func("Number.Add"));
        push_instr(&mut instrs, store_var(var));
        push_instr(&mut instrs, pop_instr());
    }

    // Emit condition + AddSaliencyCandidate for each member.
    let no_content_label = "L_NoContent".to_owned();
    for (i, member) in members.iter().enumerate() {
        let cand_label = format!("L_Cand_{i}");
        // Evaluate the `when:` expressions; result (bool) goes onto stack.
        let cond_instrs = compile_when_expressions(&member.when_expressions, &member.unique_name);
        for instr in cond_instrs {
            push_instr(&mut instrs, instr);
        }
        // AddSaliencyCandidate(content_id, complexity, dest_label)
        let complexity = compute_complexity(&member.when_expressions);
        instrs.push(Instruction {
            opcode: OpCode::AddSaliencyCandidate as i32,
            operands: vec![
                Operand::from(member.unique_name.clone()), // content_id
                Operand::from(complexity),                 // complexity score
                Operand::from(cand_label),                 // destination label
            ],
        });
    }

    // Select the best eligible candidate.
    instrs.push(Instruction {
        opcode: OpCode::SelectSaliencyCandidate as i32,
        operands: vec![],
    });

    // If no candidate was eligible, jump to L_NoContent.
    // Note: JumpIfFalse uses peek() — it does NOT pop the value.
    instrs.push(Instruction {
        opcode: OpCode::JumpIfFalse as i32,
        operands: vec![Operand::from(no_content_label.clone())],
    });

    // Pop the `true` flag left on stack by SelectSaliencyCandidate / JumpIfFalse.
    // Stack before: [dest_i32, true]. After pop: [dest_i32].
    push_instr(&mut instrs, pop_instr());

    // Peek the dest_i32 at the top and jump to the matching L_Cand_i block.
    instrs.push(Instruction {
        opcode: OpCode::PeekAndJump as i32,
        operands: vec![],
    });

    // Per-candidate dispatch blocks: L_Cand_i → Pop, PushString(unique), DetourToNode, Return
    for (i, member) in members.iter().enumerate() {
        let cand_label = format!("L_Cand_{i}");
        labels.insert(cand_label, instrs.len() as i32);
        push_instr(&mut instrs, pop_instr()); // discard the destination index PeekAndJump left on stack
        push_instr(&mut instrs, push_string(&member.unique_name));
        instrs.push(Instruction {
            opcode: OpCode::DetourToNode as i32,
            operands: vec![],
        });
        instrs.push(Instruction {
            opcode: OpCode::Return as i32,
            operands: vec![],
        });
    }

    // L_NoContent: pop the `false` left on stack by SelectSaliencyCandidate, then return.
    labels.insert(no_content_label, instrs.len() as i32);
    push_instr(&mut instrs, pop_instr()); // remove the `false` from JumpIfFalse
    instrs.push(Instruction {
        opcode: OpCode::Return as i32,
        operands: vec![],
    });

    Node {
        name: group_title.to_owned(),
        instructions: instrs,
        labels,
        tags: vec!["yarn_internal_node_group".to_owned()],
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Instruction-building helpers
// ---------------------------------------------------------------------------

#[inline]
fn push_instr(instrs: &mut Vec<Instruction>, instr: Instruction) {
    instrs.push(instr);
}

#[inline]
fn push_var(name: &str) -> Instruction {
    Instruction {
        opcode: OpCode::PushVariable as i32,
        operands: vec![Operand::from(name.to_owned())],
    }
}

#[inline]
fn store_var(name: &str) -> Instruction {
    Instruction {
        opcode: OpCode::StoreVariable as i32,
        operands: vec![Operand::from(name.to_owned())],
    }
}

#[inline]
fn push_float(v: f32) -> Instruction {
    Instruction {
        opcode: OpCode::PushFloat as i32,
        operands: vec![Operand::from(v)],
    }
}

#[inline]
fn push_bool(v: bool) -> Instruction {
    Instruction {
        opcode: OpCode::PushBool as i32,
        operands: vec![Operand::from(v)],
    }
}

#[inline]
fn push_string(s: &str) -> Instruction {
    Instruction {
        opcode: OpCode::PushString as i32,
        operands: vec![Operand::from(s.to_owned())],
    }
}

#[inline]
fn call_func(name: &str) -> Instruction {
    Instruction {
        opcode: OpCode::CallFunc as i32,
        operands: vec![Operand::from(name.to_owned())],
    }
}

#[inline]
fn pop_instr() -> Instruction {
    Instruction {
        opcode: OpCode::Pop as i32,
        operands: vec![],
    }
}

// ---------------------------------------------------------------------------
// `when:` expression compiler
// ---------------------------------------------------------------------------

/// Compile all `when:` header values for a member node into a sequence of VM
/// instructions whose net effect is to push a single `bool` onto the stack.
/// Multiple expressions are ANDed together; `always` (or empty list) → push true.
fn compile_when_expressions(exprs: &[String], content_id: &str) -> Vec<Instruction> {
    // Filter to non-always expressions that actually need code
    let non_trivial: Vec<&str> = exprs.iter().map(|e| e.trim()).filter(|s| *s != "always").collect();

    if non_trivial.is_empty() {
        // All headers are `always` (or empty) — unconditionally true.
        return vec![push_bool(true)];
    }

    let mut out = Vec::new();
    for (i, expr) in non_trivial.iter().enumerate() {
        out.extend(compile_single_when_expression(expr, content_id));
        if i > 0 {
            // AND this result with the previous one
            out.push(push_float(2.0));
            out.push(call_func("Bool.And"));
        }
    }
    out
}

/// Compile a single `when:` header value to VM instructions pushing one `bool`.
fn compile_single_when_expression(expr: &str, content_id: &str) -> Vec<Instruction> {
    let s = expr.trim();

    // "once" — true when the content has never been selected before.
    if s == "once" {
        let vc = view_count_var(content_id);
        return vec![push_var(&vc), push_float(0.0), push_float(2.0), call_func("Number.EqualTo")];
    }

    // "once if <expr>" — true when never selected AND the sub-expression is true.
    if let Some(rest) = s.strip_prefix("once if ") {
        let vc = view_count_var(content_id);
        let mut out = vec![push_var(&vc), push_float(0.0), push_float(2.0), call_func("Number.EqualTo")];
        out.extend(parse_expr_str(rest.trim()));
        out.push(push_float(2.0));
        out.push(call_func("Bool.And"));
        return out;
    }

    parse_expr_str(s)
}

/// Variable name that `SelectSaliencyCandidate` uses to track how many times
/// a given content item has been chosen.
fn view_count_var(content_id: &str) -> String {
    format!("$Yarn.Internal.Content.ViewCount.{content_id}")
}

/// Complexity score for a set of `when:` expressions, matching C# behavior.
///
/// C# algorithm (from `When_headerContext.ComplexityScore` summed over all headers):
///   - `always`            → 0
///   - `once` (no expr)   → 1
///   - `once if <expr>`   → 1 + (count_bool_ops(expr) + 1)
///   - `<expr>`           → count_bool_ops(expr) + 1
///
/// The total for the node is the sum across all `when:` headers.
fn compute_complexity(exprs: &[String]) -> f32 {
    exprs.iter().map(|e| compute_single_complexity(e.trim())).sum()
}

/// Complexity for a single `when:` header value, matching C# `When_headerContext.ComplexityScore`.
fn compute_single_complexity(s: &str) -> f32 {
    if s == "always" {
        return 0.0;
    }
    if s == "once" {
        return 1.0;
    }
    if let Some(rest) = s.strip_prefix("once if ") {
        // once (+1) + expression (bool_ops + 1)
        return 1.0 + count_bool_ops(rest.trim()) as f32 + 1.0;
    }
    // Any other expression: bool_ops + 1
    count_bool_ops(s) as f32 + 1.0
}

/// Count the number of binary boolean operators (AND / OR) in an expression.
/// This matches C# `GetBooleanOperatorCountInExpression` which counts `ExpAndOrXorContext` nodes.
fn count_bool_ops(s: &str) -> usize {
    tokenize(s).iter().filter(|t| matches!(t, WhenTok::And | WhenTok::Or)).count()
}

// ---------------------------------------------------------------------------
// Minimal recursive-descent expression parser
// ---------------------------------------------------------------------------

/// Tokenises an expression string into `WhenToken`s.
#[derive(Debug, Clone)]
enum WhenTok {
    Bool(bool),
    Float(f32),
    Var(String),
    Ident(String),
    Not,
    And,
    Or,
    Plus,
    Minus,
    Star,
    Slash,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    LParen,
    RParen,
    Comma,
    Eof,
}

fn tokenize(input: &str) -> Vec<WhenTok> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Skip whitespace
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }

        match chars[i] {
            '(' => {
                tokens.push(WhenTok::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(WhenTok::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(WhenTok::Comma);
                i += 1;
            }
            '+' => {
                tokens.push(WhenTok::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(WhenTok::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(WhenTok::Star);
                i += 1;
            }
            '/' => {
                tokens.push(WhenTok::Slash);
                i += 1;
            }
            '=' if chars.get(i + 1) == Some(&'=') => {
                tokens.push(WhenTok::EqEq);
                i += 2;
            }
            '!' if chars.get(i + 1) == Some(&'=') => {
                tokens.push(WhenTok::Ne);
                i += 2;
            }
            '&' if chars.get(i + 1) == Some(&'&') => {
                tokens.push(WhenTok::And);
                i += 2;
            }
            '|' if chars.get(i + 1) == Some(&'|') => {
                tokens.push(WhenTok::Or);
                i += 2;
            }
            '<' if chars.get(i + 1) == Some(&'=') => {
                tokens.push(WhenTok::Le);
                i += 2;
            }
            '<' => {
                tokens.push(WhenTok::Lt);
                i += 1;
            }
            '>' if chars.get(i + 1) == Some(&'=') => {
                tokens.push(WhenTok::Ge);
                i += 2;
            }
            '>' => {
                tokens.push(WhenTok::Gt);
                i += 1;
            }
            '$' => {
                // Variable reference: $ident
                i += 1;
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                tokens.push(WhenTok::Var(format!("${}", &input[start..i])));
            }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let s = &input[start..i];
                let v: f32 = s.parse().unwrap_or(0.0);
                tokens.push(WhenTok::Float(v));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word = &input[start..i];
                match word {
                    "true" | "always" => tokens.push(WhenTok::Bool(true)),
                    "false" => tokens.push(WhenTok::Bool(false)),
                    "not" => tokens.push(WhenTok::Not),
                    "and" => tokens.push(WhenTok::And),
                    "or" => tokens.push(WhenTok::Or),
                    other => tokens.push(WhenTok::Ident(other.to_owned())),
                }
            }
            _ => {
                i += 1;
            } // skip unknown chars
        }
    }
    tokens.push(WhenTok::Eof);
    tokens
}

struct WhenExprParser {
    tokens: Vec<WhenTok>,
    pos: usize,
}

impl WhenExprParser {
    fn new(tokens: Vec<WhenTok>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &WhenTok {
        &self.tokens[self.pos]
    }

    fn next(&mut self) -> WhenTok {
        let tok = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    // Grammar (simplified, no precedence climbing needed for test cases):
    //   expr       = or_expr
    //   or_expr    = and_expr ('or' and_expr)*
    //   and_expr   = cmp_expr ('and' cmp_expr)*
    //   cmp_expr   = add_expr (('==' | '!=' | '<' | '<=' | '>' | '>=') add_expr)?
    //   add_expr   = mul_expr (('+' | '-') mul_expr)*
    //   mul_expr   = unary (('*' | '/') unary)*
    //   unary      = 'not' unary | primary
    //   primary    = bool | float | var | ident ['(' args ')'] | '(' expr ')'

    /// Parse an expression; returns the Vec<Instruction> that pushes one value.
    fn parse_expr(&mut self) -> (Vec<Instruction>, ExprType) {
        self.parse_or()
    }

    fn parse_or(&mut self) -> (Vec<Instruction>, ExprType) {
        let (mut out, mut ty) = self.parse_and();
        while matches!(self.peek(), WhenTok::Or) {
            self.next();
            let (rhs, _rty) = self.parse_and();
            out.extend(rhs);
            out.push(push_float(2.0));
            out.push(call_func("Bool.Or"));
            ty = ExprType::Bool;
        }
        (out, ty)
    }

    fn parse_and(&mut self) -> (Vec<Instruction>, ExprType) {
        let (mut out, mut ty) = self.parse_cmp();
        while matches!(self.peek(), WhenTok::And) {
            self.next();
            let (rhs, _rty) = self.parse_cmp();
            out.extend(rhs);
            out.push(push_float(2.0));
            out.push(call_func("Bool.And"));
            ty = ExprType::Bool;
        }
        (out, ty)
    }

    fn parse_cmp(&mut self) -> (Vec<Instruction>, ExprType) {
        let (mut out, lty) = self.parse_add();
        let op = match self.peek() {
            WhenTok::EqEq => Some("EqualTo"),
            WhenTok::Ne => Some("NotEqualTo"),
            WhenTok::Lt => Some("LessThan"),
            WhenTok::Le => Some("LessThanOrEqualTo"),
            WhenTok::Gt => Some("GreaterThan"),
            WhenTok::Ge => Some("GreaterThanOrEqualTo"),
            _ => None,
        };
        if let Some(op_name) = op {
            self.next();
            let (rhs, rty) = self.parse_add();
            out.extend(rhs);
            out.push(push_float(2.0));
            // Pick the type prefix: if either side is numeric use Number, else Bool.
            let prefix = if lty == ExprType::Number || rty == ExprType::Number {
                "Number"
            } else {
                "Bool"
            };
            out.push(call_func(&format!("{prefix}.{op_name}")));
            return (out, ExprType::Bool);
        }
        (out, lty)
    }

    fn parse_add(&mut self) -> (Vec<Instruction>, ExprType) {
        let (mut out, mut ty) = self.parse_mul();
        loop {
            match self.peek() {
                WhenTok::Plus => {
                    self.next();
                    let (rhs, _) = self.parse_mul();
                    out.extend(rhs);
                    out.push(push_float(2.0));
                    out.push(call_func("Number.Add"));
                    ty = ExprType::Number;
                }
                WhenTok::Minus => {
                    self.next();
                    let (rhs, _) = self.parse_mul();
                    out.extend(rhs);
                    out.push(push_float(2.0));
                    out.push(call_func("Number.Subtract"));
                    ty = ExprType::Number;
                }
                _ => break,
            }
        }
        (out, ty)
    }

    fn parse_mul(&mut self) -> (Vec<Instruction>, ExprType) {
        let (mut out, mut ty) = self.parse_unary();
        loop {
            match self.peek() {
                WhenTok::Star => {
                    self.next();
                    let (rhs, _) = self.parse_unary();
                    out.extend(rhs);
                    out.push(push_float(2.0));
                    out.push(call_func("Number.Multiply"));
                    ty = ExprType::Number;
                }
                WhenTok::Slash => {
                    self.next();
                    let (rhs, _) = self.parse_unary();
                    out.extend(rhs);
                    out.push(push_float(2.0));
                    out.push(call_func("Number.Divide"));
                    ty = ExprType::Number;
                }
                _ => break,
            }
        }
        (out, ty)
    }

    fn parse_unary(&mut self) -> (Vec<Instruction>, ExprType) {
        if matches!(self.peek(), WhenTok::Not) {
            self.next();
            let (mut out, _) = self.parse_unary();
            out.push(push_float(1.0));
            out.push(call_func("Bool.Not"));
            return (out, ExprType::Bool);
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> (Vec<Instruction>, ExprType) {
        match self.next() {
            WhenTok::Bool(b) => (vec![push_bool(b)], ExprType::Bool),
            WhenTok::Float(f) => (vec![push_float(f)], ExprType::Number),
            WhenTok::Var(name) => (vec![push_var(&name)], ExprType::Unknown),
            WhenTok::Ident(name) => {
                // Function call: ident '(' args ')'
                if matches!(self.peek(), WhenTok::LParen) {
                    self.next(); // consume '('
                    let mut args: Vec<Vec<Instruction>> = Vec::new();
                    while !matches!(self.peek(), WhenTok::RParen | WhenTok::Eof) {
                        let (arg_instrs, _) = self.parse_expr();
                        args.push(arg_instrs);
                        if matches!(self.peek(), WhenTok::Comma) {
                            self.next(); // consume ','
                        }
                    }
                    self.next(); // consume ')'
                    let arg_count = args.len();
                    let mut out = Vec::new();
                    for arg in args {
                        out.extend(arg);
                    }
                    out.push(push_float(arg_count as f32));
                    out.push(call_func(&name));
                    (out, ExprType::Unknown)
                } else {
                    // Bare identifier — treat as a boolean variable reference.
                    // This shouldn't normally appear in `when:` expressions.
                    (vec![push_bool(true)], ExprType::Bool)
                }
            }
            WhenTok::LParen => {
                let (out, ty) = self.parse_expr();
                // consume ')'
                if matches!(self.peek(), WhenTok::RParen) {
                    self.next();
                }
                (out, ty)
            }
            WhenTok::Minus => {
                // Unary minus
                let (mut out, _) = self.parse_primary();
                out.push(push_float(1.0));
                out.push(call_func("Number.UnaryMinus"));
                (out, ExprType::Number)
            }
            _ => (vec![push_bool(true)], ExprType::Bool), // fallback
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExprType {
    Bool,
    Number,
    Unknown,
}

fn parse_expr_str(s: &str) -> Vec<Instruction> {
    let tokens = tokenize(s);
    let mut parser = WhenExprParser::new(tokens);
    let (instrs, _) = parser.parse_expr();
    instrs
}
