// The recording replacement the import watch (import-watch.mjs) puts in place
// of the few functions that neither of its other nets sees (review finding
// pa-4). A replacement must not change what the function does: a module that
// takes a property of the function (`util.promisify.custom`, Node's own
// symbols) or calls it with `new` must get what it would get from the
// original. So the replacement keeps the original's prototype chain, name,
// length and every own property, symbols included, and a property that is
// itself a function is replaced by a recording one that calls the original
// property. This module imports nothing.

// `record(label, args)` is called before every call of `original`, or of one
// of its function-valued properties, with the label of what was called.
export function recording(original, label, record, made = new Map()) {
  if (made.has(original)) return made.get(original);
  // A function without a `prototype` of its own (a method, an arrow, most
  // built-ins) is replaced by one without one too, so that its own keys are
  // the same; one with a `prototype` can be called with `new`, and so can
  // its replacement.
  const watched = Object.hasOwn(original, 'prototype')
    ? function (...args) {
        record(label, args);
        return new.target ? Reflect.construct(original, args, new.target) : Reflect.apply(original, this, args);
      }
    : {
        watched(...args) {
          record(label, args);
          return Reflect.apply(original, this, args);
        },
      }.watched;
  made.set(original, watched);
  Object.setPrototypeOf(watched, Object.getPrototypeOf(original));
  for (const key of Reflect.ownKeys(original)) {
    const descriptor = Object.getOwnPropertyDescriptor(original, key);
    if (typeof descriptor.value === 'function' && key !== 'prototype') {
      descriptor.value = recording(descriptor.value, `${label}.${String(key)}`, record, made);
    }
    Object.defineProperty(watched, key, descriptor);
  }
  return watched;
}

// Replaces each of `names` on `object` that is a function by its recording
// replacement, keeping the property's own attributes.
export function instrument(object, names, label, record) {
  for (const name of names) {
    const descriptor = Object.getOwnPropertyDescriptor(object, name);
    if (typeof descriptor?.value !== 'function') continue;
    descriptor.value = recording(descriptor.value, `${label}.${name}`, record);
    Object.defineProperty(object, name, descriptor);
  }
}
