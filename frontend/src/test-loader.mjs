export function resolve(specifier, context, nextResolve) {
  if (specifier === './api') return nextResolve('./api.ts', context)
  return nextResolve(specifier, context)
}
