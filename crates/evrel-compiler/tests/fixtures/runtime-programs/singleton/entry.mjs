import { first } from "./first.mjs";
import { second } from "./second.mjs";
import { count } from "./state.mjs";

__evrel.observe("module singleton", first, second, count);
