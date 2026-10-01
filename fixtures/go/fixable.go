package fixtures

import "strings"

func HasPrefix(value string) bool {
    return strings.Index(value, "go") >= 0
}
