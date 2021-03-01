use j4rs::{Instance, InvocationArg, Jvm, JvmBuilder};

use j4rs::errors as jerr;
use std::io;

use std::{thread, time};

fn main() -> jerr::Result<()> {
    // Create a JVM
    let jvm = JvmBuilder::new().build()?;

    println!("jvm init ok");
    // Create a java.lang.String instance
    let string_instance = jvm.create_instance(
        "java.lang.String",     // The Java class to create an instance for
        &Vec::new(),            // The `InvocationArg`s to use for the constructor call - empty for this example
        )?;

    // The instances returned from invocations and instantiations can be viewed as pointers to Java Objects.
    // They can be used for further Java calls.
    // For example, the following invokes the `isEmpty` method of the created java.lang.String instance
    let boolean_instance = jvm.invoke(
        &string_instance,       // The String instance created above
        "isEmpty",              // The method of the String instance to invoke
        &Vec::new(),            // The `InvocationArg`s to use for the invocation - empty for this example
        )?;

    // If we need to transform an `Instance` to Rust value, the `to_rust` should be called
    let rust_boolean: bool = jvm.to_rust(boolean_instance)?;
    println!("The isEmpty() method of the java.lang.String instance returned {}", rust_boolean);
    // The above prints:
    // The isEmpty() method of the java.lang.String instance returned true

    // Static invocation
    let _static_invocation_result = jvm.invoke_static(
        "java.lang.System",     // The Java class to invoke
        "currentTimeMillis",    // The static method of the Java class to invoke
        &Vec::new(),            // The `InvocationArg`s to use for the invocation - empty for this example
        )?;

    let mills: i64 = jvm.to_rust(_static_invocation_result)?;

    println!("curr mills: {}", mills);

    thread::sleep(time::Duration::from_secs(10));

    Ok(())
}
