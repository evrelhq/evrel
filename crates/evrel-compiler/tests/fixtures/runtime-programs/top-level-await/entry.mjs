import { events, value } from "./dependency.mjs";

events.push("entry");
__evrel.observe("top level await graph", value, events.join(","));
