// Minimal smoke test for ios-bridge, built and run on the iOS Simulator (not part of the
// CS2Analyzer app or its I9 LocalParser work — just proof that the .xcframework links and the
// C ABI works when actually running under the iphonesimulator platform, not just compiled for
// it on the host as `cargo test` does). Invoked by run-on-simulator.sh.
#include <stdio.h>
#include "ios_bridge.h"

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: %s <demo.dem> <out.json.gz>\n", argv[0]);
        return 2;
    }

    int status = cs2_parse_file(argv[1], argv[2], NULL, NULL);
    if (status != 0) {
        char *err = cs2_last_error();
        fprintf(stderr, "cs2_parse_file failed: status=%d error=%s\n", status, err ? err : "(none)");
        if (err) cs2_string_free(err);
        return 1;
    }

    printf("cs2_parse_file: ok, wrote %s\n", argv[2]);
    return 0;
}
