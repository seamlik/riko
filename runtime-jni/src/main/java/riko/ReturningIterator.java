package riko;

import io.reactivex.rxjava3.core.BackpressureStrategy;
import io.reactivex.rxjava3.core.Flowable;
import io.reactivex.rxjava3.core.FlowableOnSubscribe;
import java.util.Iterator;
import java.util.NoSuchElementException;

/**
 * JNI version of {@code riko_runtime::iterators::ReturningIterator}.
 */
public class ReturningIterator<T> extends Heaped implements Iterator<Returned<T>> {

  public ReturningIterator(final int handle) {
    super(handle);
  }

  @Override
  protected void drop() {
    __drop(handle);
  }

  private static native void __drop(int handle);

  /**
   * Unsupported operation because this requires a Rust `Seekable` but it is hard to store an
   * iterator adapter in a container.
   */
  @Override
  public boolean hasNext() {
    // TODO: Implement this!
    throw new UnsupportedOperationException("Rust iterators do not support hasNext natively.");
  }

  @Override
  public Returned<T> next() {
    assertAlive();

    final byte[] data = __next(handle);
    if (data.length == 0) {
      throw new NoSuchElementException();
    }
    return Marshaler.fromBytes(data);
  }

  private static native byte[] __next(int handle);

  /**
   * Consumes this iterator into a {@link Flowable}. The underlying iterator will be closed
   * eventually if the {@link Flowable} is subscribed to.
   */
  public Flowable<Returned<T>> toRxJavaFlowable() {
    consume();

    final FlowableOnSubscribe<Returned<T>> impl = emitter -> {
      while (true) {
        try {
          final Returned<T> next = ReturningIterator.this.next();
          if (emitter.isCancelled()) {
            break;
          } else {
            emitter.onNext(next);
          }
        } catch (final NoSuchElementException err) {
          emitter.onComplete();
          break;
        } catch (final Exception err) {
          emitter.onError(err);
          break;
        }
      }
    };
    return Flowable
        .create(impl, BackpressureStrategy.BUFFER)
        .doFinally(ReturningIterator.this::close);
  }
}
