use nephrite4_common::util;
use nephrite4_common::conf;

use j4rs::{Instance, InvocationArg, ClasspathEntry, Jvm, JvmBuilder};

use log::{debug, info};
use std::{io::Read, os::unix::prelude::{AsRawFd, IntoRawFd}};
use std::thread;

use crate::error::*;

use std::convert::TryFrom;

pub struct Tika {
    jvm: Jvm,
}

fn dup(jvm: &Jvm, i: &Instance) -> Instance {
    let kls = i.class_name().to_string();
    jvm.cast(i, &kls).unwrap()
}

fn ja_int(i: i32) -> InvocationArg {
    InvocationArg::try_from(i).unwrap().into_primitive().unwrap()
}

fn ja_inst(i: Instance) -> InvocationArg {
    InvocationArg::from(i)
}

fn ja_bool(i: bool) -> InvocationArg {
    InvocationArg::try_from(i).unwrap().into_primitive().unwrap()
}

impl Tika {
    pub fn new(conf: &conf::Conf) -> Result<Tika> {
        let jar = conf.tika_jar();
        let jvm: Jvm = JvmBuilder::new()
            .classpath_entry(ClasspathEntry::new(&jar))
            .build()?;

        info!("tika jvm init done");

        Ok(Tika { jvm })
    }

    pub fn parse_file(&self, path: &str) -> Result<String> {
        let jvm = &self.jvm;

        let input = jvm.create_instance(
            "java.io.FileInputStream",
            &vec![InvocationArg::try_from(path)?],
            )?;

        Tika::parse_stream(&jvm, input)
    }

    fn parse_stream(jvm: &Jvm, input: Instance) -> Result<String> {
        /*
        private ContentHandlerFactory getContentHandlerFactory(OutputType type) {
        BasicContentHandlerFactory.HANDLER_TYPE handlerType = BasicContentHandlerFactory.HANDLER_TYPE.IGNORE;
        if (type.equals(HTML)) {
        handlerType = BasicContentHandlerFactory.HANDLER_TYPE.HTML;
    } else if (type.equals(XML)) {
        handlerType = BasicContentHandlerFactory.HANDLER_TYPE.XML;
    } else if (type.equals(TEXT)) {
        handlerType = BasicContentHandlerFactory.HANDLER_TYPE.TEXT;
    } else if (type.equals(TEXT_MAIN)) { // <---- NOTE here
        handlerType = BasicContentHandlerFactory.HANDLER_TYPE.BODY;
    } else if (type.equals(METADATA)) {
        handlerType = BasicContentHandlerFactory.HANDLER_TYPE.IGNORE;
    }
        return new BasicContentHandlerFactory(handlerType, -1);
    }

        Metadata metadata = new Metadata();
        RecursiveParserWrapper wrapper = new RecursiveParserWrapper(parser);
        RecursiveParserWrapperHandler handler =
                new RecursiveParserWrapperHandler(getContentHandlerFactory(type),
                        -1, config.getMetadataFilter());
        try (InputStream input = TikaInputStream.get(url, metadata)) {
            wrapper.parse(input, handler, metadata, context);
        }
        JsonMetadataList.setPrettyPrinting(prettyPrint);
        Writer writer = getOutputWriter(output, encoding);
        JsonMetadataList.toJson(handler.getMetadataList(), writer);

    }
         */
        // init
        let config = jvm.invoke_static(
            "org.apache.tika.config.TikaConfig",
            "getDefaultConfig",
            &Vec::new())?;

        let parser = jvm.create_instance(
            "org.apache.tika.parser.AutoDetectParser",
            &Vec::new())?;

        let context = jvm.create_instance(
            "org.apache.tika.parser.ParseContext",
            &Vec::new())?;

        //let (parser, parser1) = dup_inst(&jvm, parser);

        let parser_kls = jvm.invoke(&parser, "getClass", &Vec::new())?;

        jvm.invoke(&context, "set",
                   &vec![ja_inst(parser_kls), ja_inst(dup(jvm, &parser))])?;

        //
        let metadata = jvm.create_instance(
            "org.apache.tika.metadata.Metadata",
            &Vec::new())?;

        let tp = jvm.static_class_field(
            "org.apache.tika.sax.BasicContentHandlerFactory$HANDLER_TYPE",
            "BODY")?;

        let handler_fac = jvm.create_instance(
            "org.apache.tika.sax.BasicContentHandlerFactory",
            &vec![ja_inst(tp), ja_int(-1)])?; // .BODY

        let mf = jvm.invoke(&config, "getMetadataFilter", &Vec::new())?;

        let wrapper = jvm.create_instance(
            "org.apache.tika.parser.RecursiveParserWrapper",
            &vec![ja_inst(parser)])?;

        let handler = jvm.create_instance(
            "org.apache.tika.sax.RecursiveParserWrapperHandler",
            &vec![ja_inst(handler_fac),
                  ja_int(-1),
                  ja_inst(mf)])?;


        jvm.invoke(&wrapper, "parse",
                   &vec![ja_inst(input),
                         ja_inst(dup(jvm, &handler)),
                         ja_inst(metadata),
                         ja_inst(context)])?;


        jvm.invoke_static(
            "org.apache.tika.metadata.serialization.JsonMetadataList",
            "setPrettyPrinting", &vec![ja_bool(false)])?;

        let writter = jvm.create_instance(
            "java.io.CharArrayWriter",
            &vec![])?;

        let ml = jvm.invoke(&handler,
                            "getMetadataList", &vec![])?;

        jvm.invoke_static(
            "org.apache.tika.metadata.serialization.JsonMetadataList",
            "toJson",
            &vec![ja_inst(ml), ja_inst(dup(jvm, &writter))])?;

        let res_j = jvm.invoke(&writter, "toString", &Vec::new())?;

        let res = jvm.to_rust(res_j)?;

        Ok(res)
    }

    pub fn parse_from_fd<T: AsRawFd>(&self, fd: T) -> Result<String> {
        let jvm = &self.jvm;

        let path = format!("/dev/fd/{}", fd.as_raw_fd());
        let input = jvm.create_instance(
            "java.io.FileInputStream",
            &vec![InvocationArg::try_from(path)?],
            )?;

        let res = Tika::parse_stream(&jvm, dup(jvm, &input))?;
        jvm.invoke(&input, "close", &vec![])?;

        Ok(res)
    }
}
