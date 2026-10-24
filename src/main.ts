import { mount } from "svelte";
// Global tokens and fonts first, so app.css precedes component CSS in the cascade.
import "./app.css";
import App from "./App.svelte";

const target = document.getElementById("app");
if (!target) throw new Error("index.html is missing the #app mount target");

export default mount(App, { target });
