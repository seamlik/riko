package riko;

/** Rust object allocated on the heap. */
public class Object implements AutoCloseable {

  protected final int handle;

  public Object(final int handle) {
    this.handle = handle;
  }

  @Override
  public void close() {
    drop(handle);
  }

  private static native void drop(int handle);

  private static native boolean aliveNative(int handle);

  public boolean alive() {
    return aliveNative(handle);
  }
}
