# Region helpers. `create`, `put`, and `free` are compiler intrinsics
# resolved when this module is required (e.g. `"std.region" region
# require`). They wrap the host region registry:
#   create  → region handle (i64)
#   put     → attach a heap value; region owns it
#   free    → free every attached value, then the region
