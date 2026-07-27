const events = [];

outer: for (let outer = 0; outer < 3; outer++) {
    for (let inner = 0; inner < 3; inner++) {
        try {
            events.push(`try:${outer}:${inner}`);
            if (outer === 0 && inner === 1) continue outer;
            if (outer === 1 && inner === 1) break outer;
        } finally {
            events.push(`finally:${outer}:${inner}`);
        }
    }
}

__evrel.observe("labeled finally", events.join(","));
