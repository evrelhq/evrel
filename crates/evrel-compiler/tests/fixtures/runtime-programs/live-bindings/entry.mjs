import { increment, value } from "./state.mjs";

__evrel.observe("live binding before", value);
increment();
__evrel.observe("live binding after", value);
