use nephrite4_common::util;
use nephrite4_common::conf;

use j4rs::{Instance, InvocationArg, ClasspathEntry, Jvm, JvmBuilder};

use log::{debug, info};

use crate::error::*;

use std::convert::TryFrom;

use xml::reader::{EventReader, XmlEvent};


pub struct Tika {
    jvm: Jvm,
}

fn dup_inst(jvm: &Jvm, i: Instance) -> (Instance, Instance) {
    let kls = i.class_name().to_string();

    debug!("dup inst in {}", &kls);

    let n = InvocationArg::from(i);

    let al = jvm.create_instance(
        "java.util.ArrayList", &vec![])
        .unwrap();

    jvm.invoke(&al, "add", &vec![n]).unwrap();

    let z0 = InvocationArg::try_from(0).unwrap().into_primitive().unwrap();
    let r0 = jvm.invoke(&al, "get", &vec![z0]).unwrap();
    let z1 = InvocationArg::try_from(0).unwrap().into_primitive().unwrap();
    let r1 = jvm.invoke(&al, "get", &vec![z1]).unwrap();

    let r0c = jvm.cast(&r0, &kls).unwrap();
    let r1c = jvm.cast(&r1, &kls).unwrap();

    debug!("dup inst out {}", &r0c.class_name());

    (r0c, r1c)
}

impl Tika {
    pub fn new(conf: &conf::Conf) -> Result<Tika> {
        let jar = conf.tika_jar();
        let jvm: Jvm = JvmBuilder::new()
            .classpath_entry(ClasspathEntry::new(&jar))
            .build()?;

        let jvm = JvmBuilder::new().build()?;

        info!("tika jvm init done");


        Ok(Tika { jvm })
    }

    pub fn parse(&self, path: &str) -> Result<String> {
        /*
        ContentHandler handler = new ToXMLContentHandler();

        AutoDetectParser parser = new AutoDetectParser();
        Metadata metadata = new Metadata();
        try (InputStream stream = ContentHandlerExample.class.getResourceAsStream("test.doc")) {
        parser.parse(stream, handler, metadata);
        return handler.toString();
    }
         */
        let jvm = &self.jvm;

        let handler = jvm.create_instance(
            "org.apache.tika.sax.ToXMLContentHandler",
            &Vec::new())?;

        let parser = jvm.create_instance(
            "org.apache.tika.parser.AutoDetectParser",
            &Vec::new())?;

        let metadata = jvm.create_instance(
            "org.apache.tika.metadata.Metadata",
            &Vec::new())?;

        let stream = jvm.create_instance(
            "java.io.FileInputStream",
            &vec![InvocationArg::try_from(path)?],
            )?;

        let (handler, handler_dup) = dup_inst(jvm, handler);

        jvm.invoke(&parser, "parse",
                   &vec![InvocationArg::from(stream),
                         InvocationArg::from(handler),
                         InvocationArg::from(metadata)])?;

        let res_j = jvm.invoke(&handler_dup, "toString", &Vec::new())?;

        let res = jvm.to_rust(res_j)?;

        Ok(res)
    }
}
