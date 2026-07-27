import "./first.mjs";
import "./second.mjs";
import { events } from "./state.mjs";

events.push("entry");
__evrel.observe("module evaluation order", events.join(","));
