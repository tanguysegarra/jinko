//! The VarAssign struct is used when assigning values to variables.

use crate::instruction::{InstrKind, Var};
use crate::typechecker::{CheckedType, TypeCtx};
use crate::Generic;
use crate::{log, Context, ErrKind, Error, Instruction, ObjectInstance, TypeCheck};

#[derive(Clone)]
pub struct VarAssign {
    /// Is the variable mutable ? This is only useful on variable declaration
    mutable: bool,

    /// The "name" of the variable
    symbol: String,

    value: Box<dyn Instruction>,
    typechecked: bool,
}

impl VarAssign {
    pub fn new(mutable: bool, symbol: String, value: Box<dyn Instruction>) -> VarAssign {
        VarAssign {
            mutable,
            symbol,
            value,
            typechecked: false,
        }
    }

    /// Get a reference to the symbol of the variable assignment
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Is a variable is declared as mutable or not
    pub fn mutable(&self) -> bool {
        self.mutable
    }

    /// Get a reference to the value used to initialize the variable
    pub fn value(&self) -> &dyn Instruction {
        &*self.value
    }

    /// Get a mutable reference to the value used to initialize the variable
    pub fn value_mut(&mut self) -> &mut dyn Instruction {
        &mut *self.value
    }
}

impl Instruction for VarAssign {
    fn kind(&self) -> InstrKind {
        InstrKind::Statement
    }

    fn print(&self) -> String {
        let base = if self.mutable {
            String::from("mut ")
        } else {
            String::new()
        };
        format!("{}{} = {}", base, self.symbol, self.value.print())
    }

    fn execute(&self, ctx: &mut Context) -> Option<ObjectInstance> {
        log!("assign var: {}", self.symbol());

        // Are we creating the variable or not
        let mut var_creation = false;

        let mut var = match ctx.get_variable(&self.symbol) {
            Some(v) => v.clone(),
            None => {
                let mut new_v = Var::new(self.symbol().to_string());
                new_v.set_mutable(self.mutable());

                var_creation = true;

                new_v
            }
        };

        match (var_creation, var.mutable()) {
            (false, false) => {
                // The variable already exists. So we need to error out if it isn't
                // mutable
                ctx.error(Error::new(ErrKind::Context).with_msg(format!(
                    "trying to assign value to non mutable variable `{}`: `{}`",
                    var.name(),
                    self.value.print()
                )));
                return None;
            }
            (true, _) | (_, true) => var.set_instance(self.value.execute_expression(ctx)?),
        }

        // We can unwrap safely since we checked that the variable does not
        // exist
        ctx.replace_variable(var).unwrap();

        // A variable assignment is always a statement
        None
    }
}

impl TypeCheck for VarAssign {
    fn resolve_type(&mut self, ctx: &mut TypeCtx) -> CheckedType {
        let second_pass = ctx.is_second_pass();
        let var_ty = match ctx.get_var(&self.symbol) {
            // FIXME: Remove clone?
            Some(checked_ty) => {
                // If `self` is mutable, then it means that we are creating the variable
                // for the first time. However, we entered the match arm because the variable
                // is already present in the context. Error out appropriately.
                // In the second pass of the typechecker however, this is an appropriate
                // behavior... Which is a bit annoying to handle.
                // TODO: Think about removing the call to `is_second_pass()`
                if self.mutable() && !second_pass {
                    let err_msg = format!(
                        "trying to redefine already defined variable: {}",
                        self.symbol()
                    );
                    ctx.error(Error::new(ErrKind::TypeChecker).with_msg(err_msg));
                    return CheckedType::Unknown;
                }

                checked_ty.clone()
            }
            None => {
                let instance_ty = self.value.type_of(ctx);
                if let Err(e) = ctx.declare_var(self.symbol.clone(), instance_ty) {
                    ctx.error(e);
                }

                // We can return here since it's a new variable. This avoids checking
                // the type later on
                return CheckedType::Void;
            }
        };

        // FIXME: We resolve the value twice
        let value_ty = self.value.type_of(ctx);
        if value_ty == CheckedType::Void {
            ctx.error(Error::new(ErrKind::TypeChecker).with_msg(format!(
                "trying to assign statement `{}` to variable `{}`",
                self.value().print(),
                self.symbol
            )));
            return CheckedType::Unknown;
        }

        if var_ty != value_ty {
            ctx.error(Error::new(ErrKind::TypeChecker).with_msg(format!(
                "trying to assign value of types `{}` to variable of type `{}`",
                value_ty, var_ty
            )));
            return CheckedType::Unknown;
        }

        CheckedType::Void
    }

    fn set_cached_type(&mut self, _ty: CheckedType) {
        self.typechecked = true;
    }

    fn cached_type(&self) -> Option<&CheckedType> {
        match self.typechecked {
            true => Some(&CheckedType::Void),
            false => None,
        }
    }
}

impl Generic for VarAssign {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::constructs;
    use crate::value::{JkInt, JkString};
    use crate::ToObjectInstance;
    use crate::{jinko, jinko_fail};

    #[test]
    fn non_mutable() {
        let var_assignment = VarAssign::new(false, "x".to_owned(), Box::new(JkInt::from(12)));

        assert_eq!(var_assignment.print(), "x = 12");
    }

    #[test]
    fn mutable() {
        let var_assignment = VarAssign::new(
            true,
            "some_id_99".to_owned(),
            Box::new(JkString::from("Hey there")),
        );

        assert_eq!(var_assignment.print(), "mut some_id_99 = \"Hey there\"");
    }

    #[test]
    fn assign_mutable() {
        let mut i = Context::new();
        let va_init = constructs::expr("mut a = 13").unwrap().1;
        let va_0 = constructs::expr("a = 15").unwrap().1;

        va_init.execute(&mut i);
        va_0.execute(&mut i);

        let va_get = constructs::expr("a").unwrap().1;
        assert_eq!(
            va_get.execute(&mut i).unwrap(),
            JkInt::from(15).to_instance()
        );
    }

    #[test]
    fn assign_immutable() {
        let mut i = Context::new();
        let va_init = constructs::expr("a = 13").unwrap().1;
        let va_0 = constructs::expr("a = 15").unwrap().1;

        va_init.execute(&mut i);
        if va_0.execute(&mut i).is_some() {
            unreachable!("Can't assign twice to immutable variables");
        }

        assert!(i.error_handler.has_errors());
    }

    #[test]
    fn create_mutable_twice() {
        jinko_fail! {
            mut a0 = 15;
            mut a0 = 14;
        };
    }

    #[test]
    fn assign_mutable_in_block_187() {
        let ctx = jinko! {
            mut x = 1;
            {
                x = 0;
            }
        };

        assert_eq!(
            ctx.get_variable("x").unwrap().instance(),
            JkInt::from(0).to_instance()
        );
    }

    #[test]
    fn assign_mutable_in_function_187() {
        let ctx = jinko! {
            mut x = 1;
            func change_global() {
                x = 0;
            }

            change_global()
        };

        assert_eq!(
            ctx.get_variable("x").unwrap().instance(),
            JkInt::from(0).to_instance()
        );
    }
}
