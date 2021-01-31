package riko

import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionStage
import java.util.concurrent.Future as StdFuture
import org.bson.BsonValue

/** Analogous to a Rust `Future`. */
class Future
private constructor(
    private val inner: CompletableFuture<BsonValue>,
    private val handle: Handle,
) : StdFuture<BsonValue> by inner, CompletionStage<BsonValue> by inner {

  constructor(handle: Handle) : this(CompletableFuture(), handle)

  init {
    synchronized(pool) {
      pool[handle] = this
      notifyCompletion()
    }
  }

  override fun cancel(mayInterruptIfRunning: Boolean): Boolean {
    Companion.cancel(handle)
    synchronized(lock) {
      pool.remove(handle)
      completed.remove(handle)
    }
    return inner.cancel(mayInterruptIfRunning)
  }

  companion object {

    private val lock = Any()
    private val pool = mutableMapOf<Handle, Future>()
    private val completed = mutableMapOf<Handle, Returned>()

    private fun notifyCompletion() {
      synchronized(lock) {
        for (entry in completed.entries.filter { pool.containsKey(it.key) }) {
          val future = pool[entry.key]!!

          val value: BsonValue
          try {
            value = entry.value.unwrap()
          } catch (e: Exception) {
            future.inner.completeExceptionally(e)
            return
          }

          future.inner.complete(value)
        }
      }
    }

    /** Notifies that a Rust `Future` has completed. */
    @JvmStatic
    private fun complete(handle: Handle, raw: ByteArray) {
      synchronized(lock) {
        completed[handle] = Marshaler.decode(raw)
        notifyCompletion()
      }
    }

    @JvmStatic private external fun cancel(handle: Handle)
  }
}
