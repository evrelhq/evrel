var result = delete runtimeProbeMissingName;

__evrel.observe(
    "delete unresolvable reference",
    result,
    typeof runtimeProbeMissingName,
);
