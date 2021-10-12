//! The FFI module allows the jinko ctx to call into native code.
//! Primitive types are converted to their C counterparts.
//! FIXME

use crate::instruction::FunctionCall;
use crate::{Context, ErrKind, Error, ObjectInstance};

pub struct JkFfi;

impl JkFfi {
    pub fn execute(call: &FunctionCall, ctx: &mut Context) -> Option<ObjectInstance> {
        ctx.debug("EXT CALL", call.name());

        for lib in ctx.libs().iter() {
            let sym = call.name().as_bytes();
            unsafe {
                if lib.get::<libloading::Symbol<()>>(&sym).is_ok() {
                    match call.args().len() {
                        0 => {
                            if let Ok(f) =
                                lib.get::<libloading::Symbol<fn() -> ObjectInstance>>(&sym)
                            {
                                return Some(f());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        ctx.error(
            Error::new(ErrKind::ExternFunc)
                .with_msg(format!("cannot call external function {}", call.name())),
        );

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Construct;
    use crate::{JkInt, ToObjectInstance};

    fn declare_ext_funcs(i: &mut Context) {
        let dec0 = Construct::instruction("ext func no_arg() -> int;")
            .unwrap()
            .1;
        let dec1 = Construct::instruction("ext func square(v: int) -> int;")
            .unwrap()
            .1;
        let dec2 = Construct::instruction("ext func add(lhs: int, rhs: int) -> int;")
            .unwrap()
            .1;

        dec0.execute(i).unwrap();
        dec1.execute(i).unwrap();
        dec2.execute(i).unwrap();
    }

    fn init_ctx() -> Context {
        let mut i = Context::new();

        i.add_lib(unsafe { libloading::Library::new("./tests/fixtures/clib/lib.so").unwrap() });

        declare_ext_funcs(&mut i);

        i
    }

    #[test]
    fn t_load() {
        let _ = init_ctx();
    }

    #[test]
    #[ignore]
    fn t_no_arg() {
        let mut i = init_ctx();

        let call = Construct::instruction("no_arg()").unwrap().1;
        let call = call.downcast_ref::<FunctionCall>().unwrap();

        assert_eq!(
            JkFfi::execute(&call, &mut i),
            Some(JkInt::from(15).to_instance())
        );
    }

    #[test]
    #[ignore]
    fn t_one_arg() {
        let mut i = init_ctx();

        let call = Construct::instruction("add(12, 15)").unwrap().1;
        let call = call.downcast_ref::<FunctionCall>().unwrap();

        assert_eq!(
            JkFfi::execute(&call, &mut i),
            Some(JkInt::from(27).to_instance())
        );
    }
}
