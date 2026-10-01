#include <stddef.h>

size_t length(const char *value) {
    return value ? 1u : 0u;
}
