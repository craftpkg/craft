use predicates::prelude::*;
use predicates::str::contains;
use serde_json::json;

use crate::common::World;

const RENDER_WITH_STORE_AND_QUERIES: &str = r#"
const React = require('react');
const { renderToString } = require('react-dom/server');
const { Provider, useSelector } = require('react-redux');
const { configureStore, createSlice } = require('@reduxjs/toolkit');
const { QueryClient, QueryClientProvider, useQueryClient } = require('@tanstack/react-query');

const greeting = createSlice({ name: 'greeting', initialState: 'hello', reducers: {} });
const store = configureStore({ reducer: { greeting: greeting.reducer } });
function Greeting() {
  const text = useSelector((state) => state.greeting);
  const client = useQueryClient();
  return React.createElement('p', null, `${text} with ${client ? 'queries' : 'nothing'}`);
}
const html = renderToString(
  React.createElement(Provider, { store },
    React.createElement(QueryClientProvider, { client: new QueryClient() },
      React.createElement(Greeting))));
const reactSeenBy = (pkg) =>
  require.resolve('react', { paths: [require('path').dirname(require.resolve(pkg))] });
const copies = new Set(['react-dom', 'react-redux', '@tanstack/react-query'].map(reactSeenBy));
copies.add(require.resolve('react'));
console.log(html, copies.size);
"#;

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn react_renders_with_peer_dependent_libraries_sharing_one_react() {
    let world = World::new(&json!({
        "dependencies": {
            "react": "19.1.0",
            "react-dom": "19.1.0",
            "react-redux": "9.2.0",
            "@reduxjs/toolkit": "2.5.0",
            "@tanstack/react-query": "5.62.0",
        },
    }));

    world
        .craft()
        .arg("install")
        .assert()
        .success()
        .stderr(contains("wants peer").not());

    assert_eq!(
        world.node(RENDER_WITH_STORE_AND_QUERIES),
        "<p>hello with queries</p> 1\n"
    );
    assert!(
        world
            .virtual_store()
            .iter()
            .any(|entry| entry.starts_with("react-redux@9.2.0_"))
    );
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn warns_when_a_peer_range_is_not_met() {
    let world = World::new(&json!({
        "dependencies": { "react": "17.0.2", "react-redux": "9.2.0" },
    }));

    world
        .craft()
        .arg("install")
        .assert()
        .success()
        .stderr(contains("react-redux@9.2.0 wants peer react@").and(contains("but gets 17.0.2")));
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn a_scaffolded_vite_project_installs_and_builds() {
    let world = World::new(&json!({}));

    world
        .craft()
        .args([
            "dlx",
            "create-vite@6.5.0",
            "app",
            "--template",
            "vanilla-ts",
        ])
        .assert()
        .success()
        .stdout(contains("craft install"));
    world.craft_in("app").arg("install").assert().success();
    world
        .craft_in("app")
        .args(["run", "build"])
        .assert()
        .success();

    assert!(world.path().join("app/dist/index.html").exists());
    let assets = std::fs::read_dir(world.path().join("app/dist/assets"))
        .expect("vite writes assets")
        .count();
    assert!(assets > 0);
}

#[test]
#[ignore = "needs registry.npmjs.org and GitHub; run with `make real-world`"]
fn typescript_type_checks_through_its_bins() {
    let world = World::new(&json!({ "name": "typed" }));
    world
        .craft()
        .args(["add", "-D", "typescript@5.6.3", "@types/node@22.10.0"])
        .assert()
        .success();
    world.write(
        "index.ts",
        "import { join } from 'node:path';\nexport const where: string = join('a', 'b');\n",
    );

    world
        .craft()
        .args(["tsc", "--version"])
        .assert()
        .success()
        .stdout("Version 5.6.3\n");
    world
        .craft()
        .args(["exec", "tsc", "--noEmit", "--types", "node", "index.ts"])
        .assert()
        .success();

    world.write("broken.ts", "export const count: number = 'one';\n");
    world
        .craft()
        .args(["exec", "tsc", "--noEmit", "broken.ts"])
        .assert()
        .code(2)
        .stdout(contains("TS2322"));
}
