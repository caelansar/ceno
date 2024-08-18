use std::collections::HashMap;

use anyhow::Result;
use axum::{body::Body, response::Response};
use ceno_macros::{FromJs, IntoJs};
use rquickjs::{Context, Function, Object, Promise, Runtime};
use tracing::{info_span, instrument};
use ts_rs::TS;
use typed_builder::TypedBuilder;

#[allow(unused)]
pub struct JsWorker {
    ctx: Context,
}

#[derive(Debug, TypedBuilder, TS, IntoJs)]
pub struct Req {
    #[builder(setter(into))]
    pub method: String,
    #[builder(setter(into))]
    pub url: String,
    #[builder(default)]
    pub query: HashMap<String, String>,
    #[builder(default)]
    pub params: HashMap<String, String>,
    #[builder(default)]
    pub headers: HashMap<String, String>,
    #[builder(default)]
    pub body: Option<String>,
}

#[derive(Debug, TS, FromJs)]
pub struct Res {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

impl From<Res> for Response {
    fn from(res: Res) -> Self {
        let mut builder = Response::builder().status(res.status);
        for (k, v) in res.headers {
            builder = builder.header(k, v);
        }
        if let Some(body) = res.body {
            builder.body(body.into()).unwrap()
        } else {
            builder.body(Body::empty()).unwrap()
        }
    }
}

fn print(msg: String) {
    println!("{msg}");
}

impl JsWorker {
    #[instrument]
    pub fn try_new(module: &str) -> Result<Self> {
        let span = info_span!("init runtime");
        let _enter = span.enter();

        let rt = Runtime::new()?;
        let ctx = Context::full(&rt)?;

        drop(_enter);

        let span = info_span!("runtime ctx with");
        let _enter = span.enter();

        ctx.with(|ctx| {
            let global = ctx.globals();
            let ret: Object = ctx.eval(module)?;
            global.set("handlers", ret)?;
            // setup print function
            let fun = Function::new(ctx.clone(), print)?.with_name("rust_print")?;
            global.set("rust_print", fun)?;

            Ok::<_, anyhow::Error>(())
        })?;

        Ok(Self { ctx })
    }

    #[instrument(skip(self))]
    pub fn run(&self, name: &str, req: Req) -> anyhow::Result<Res> {
        self.ctx.with(|ctx| {
            let global = ctx.globals();
            let handlers: Object = global.get("handlers")?;
            let fun: Function = handlers.get(name)?;
            let v: Promise = fun.call((req,))?;

            Ok::<_, anyhow::Error>(v.finish()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn js_worker_should_run() {
        let code = r#"
    (function(){
        async function hello(req){
            return {
                status:200,
                headers:{
                    "content-type":"application/json"
                },
                body: JSON.stringify(req),
            };
        }
        return{hello:hello};
    })();
    "#;
        let req = Req::builder()
            .method("GET")
            .url("https://example.com")
            .headers(HashMap::new())
            .build();
        let worker = JsWorker::try_new(code).unwrap();
        let ret = worker.run("hello", req).unwrap();
        assert_eq!(ret.status, 200);
    }
}
