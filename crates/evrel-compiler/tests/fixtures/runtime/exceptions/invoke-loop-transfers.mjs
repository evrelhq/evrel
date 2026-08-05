const events = [];

try {
    for (; missingForTest; ) {
    }
} catch (error) {
    events.push(`for:${error.name}`);
}

try {
    while (missingLoopTest) {
        if (missingBranch) {
            missingTarget = 0;
            continue;
        }
    }
} catch (error) {
    events.push(`continue:${error.name}`);
}

try {
    do {
    } while (missingDoWhileTest);
} catch (error) {
    events.push(`do-while:${error.name}`);
}

__evrel.observe("invoke loop transfers", events.join(","));
