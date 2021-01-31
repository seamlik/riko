package riko

import java.util.concurrent.atomic.AtomicBoolean

/** Analogous to `riko_runtime::Handle`. */
typealias Handle = Int

/** Initializes Riko runtime. */
class Initializer {
  companion object {
    private val done = AtomicBoolean(false)

    /** Initializes Riko runtime for JNI at most once. */
    @JvmStatic
    fun initialize() {
      if (!done.getAndSet(true)) {
        __riko_initialize()
      }
    }

    @JvmStatic private external fun __riko_initialize()
  }
}
