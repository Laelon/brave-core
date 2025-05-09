use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::process::Command;

use anyhow::{Context, Error};

use crate::Cli;
use crate::Tests;

// depends on the variable 'wasm' and initializes te WasmBindgenTestContext cx
pub const SHARED_SETUP: &str = r#"
const handlers = {};

const wrap = method => {
    const og = console[method];
    const on_method = `on_console_${method}`;
    console[method] = function (...args) {
        if (nocapture) {
            og.apply(this, args);
        }
        if (handlers[on_method]) {
            handlers[on_method](args);
        }
    };
};

// save original `console.log`
global.__wbgtest_og_console_log = console.log;
// override `console.log` and `console.error` etc... before we import tests to
// ensure they're bound correctly in wasm. This'll allow us to intercept
// all these calls and capture the output of tests
wrap("debug");
wrap("log");
wrap("info");
wrap("warn");
wrap("error");

const cx = new wasm.WasmBindgenTestContext();
handlers.on_console_debug = wasm.__wbgtest_console_debug;
handlers.on_console_log = wasm.__wbgtest_console_log;
handlers.on_console_info = wasm.__wbgtest_console_info;
handlers.on_console_warn = wasm.__wbgtest_console_warn;
handlers.on_console_error = wasm.__wbgtest_console_error;
"#;

pub fn execute(
    module: &str,
    tmpdir: &Path,
    cli: Cli,
    tests: Tests,
    module_format: bool,
    coverage: PathBuf,
) -> Result<(), Error> {
    let mut js_to_execute = format!(
        r#"
        {exit};
        {fs};
        {wasm};

        const nocapture = {nocapture};
        {console_override}

        global.__wbg_test_invoke = f => f();

        async function main(tests) {{
            {args}

            const ok = await cx.run(tests.map(n => wasm.__wasm[n]));

            const coverage = wasm.__wbgtest_cov_dump();
            if (coverage !== undefined)
                await fs.writeFile('{coverage}', coverage);

            if (!ok)
                exit(1);
        }}

        const tests = [];
    "#,
        wasm = if !module_format {
            format!(r"const wasm = require('./{0}.js')", module)
        } else {
            format!(r"import * as wasm from './{0}.js'", module)
        },
        exit = if !module_format {
            r"const { exit } = require('node:process')".to_string()
        } else {
            r"import { exit } from 'node:process'".to_string()
        },
        fs = if !module_format {
            r"const fs = require('node:fs/promises')".to_string()
        } else {
            r"import fs from 'node:fs/promises'".to_string()
        },
        coverage = coverage.display(),
        nocapture = cli.nocapture.clone(),
        console_override = SHARED_SETUP,
        args = cli.into_args(&tests),
    );

    // Note that we're collecting *JS objects* that represent the functions to
    // execute, and then those objects are passed into Wasm for it to execute
    // when it sees fit.
    for test in tests.tests {
        js_to_execute.push_str(&format!("tests.push('{}')\n", test.name));
    }
    // And as a final addendum, exit with a nonzero code if any tests fail.
    js_to_execute.push_str(
        "
        main(tests)
            .catch(e => {
                console.error(e);
                exit(1);
            });
    ",
    );

    let js_path = if module_format {
        // fixme: this is a hack to make node understand modules
        let package_json = tmpdir.join("package.json");
        fs::write(&package_json, r#"{"type": "module"}"#).unwrap();
        tmpdir.join("run.mjs")
    } else {
        tmpdir.join("run.cjs")
    };
    fs::write(&js_path, js_to_execute).context("failed to write JS file")?;

    // Augment `NODE_PATH` so things like `require("tests/my-custom.js")` work
    // and Rust code can import from custom JS shims. This is a bit of a hack
    // and should probably be removed at some point.
    let path = env::var("NODE_PATH").unwrap_or_default();
    let mut path = env::split_paths(&path).collect::<Vec<_>>();
    path.push(env::current_dir().unwrap());
    path.push(tmpdir.to_path_buf());
    let extra_node_args = env::var("NODE_ARGS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    let status = Command::new("node")
        .env("NODE_PATH", env::join_paths(&path).unwrap())
        .arg("--expose-gc")
        .args(&extra_node_args)
        .arg(&js_path)
        .status()?;

    if !status.success() {
        process::exit(status.code().unwrap_or(1))
    } else {
        Ok(())
    }
}
