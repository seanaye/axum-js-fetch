import init, { MyApp } from './pkg/deno_example.js';

await init()
const app = MyApp.new()
Deno.serve((r) => app.serve(r))



