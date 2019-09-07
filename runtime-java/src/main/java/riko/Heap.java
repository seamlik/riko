package riko;

/**
 * Rust object allocated on the heap.
 */
public abstract class Heap implements AutoCloseable {

  protected final int handle;
  private boolean freed = false;

  protected Heap(final int handle) {
    this.handle = handle;
  }

  @Override
  public void close() {
    if (!freed) {
      drop();
      freed = true;
    }
  }

  /**
   * De-allocates the object on the Rust side. Should be implemented by calling a corresponding
   * native method.
   */
  protected abstract void drop();

  /**
   * Checks if the object is still alive on the Rust side. This method should be used at the
   * beginning of every instance method that requires a live Rust object on the heap.
   *
   * @throws AssertionError If the object is freed.
   */
  protected void assertNotFreed() {
    assert !freed : "Attempt of use after free.";
  }
}
