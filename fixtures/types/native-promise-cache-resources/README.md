# Promised and awaited cache backing

Distinct Promise references in a flat object initializer force 16/64 separate
promised/awaited type slots, with shallow final number results. Standard Promise
and PromiseLike declarations are intentionally minimal and shared with the
existing native await-context fixture. Four profiles vary strictNullChecks and
noCheck. Resource totals are logical request accounting, not RSS or release-time
calibration measurements.
