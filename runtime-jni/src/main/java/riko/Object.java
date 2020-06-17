package riko;

/** Rust object allocated on the heap. */
public class Object implements AutoCloseable {

  protected final int handle;

  public Object(final int handle) {
    this.handle = handle;
  }

  @Override
  public native void close();

  public native boolean alive();
}
