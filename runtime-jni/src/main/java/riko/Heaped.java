package riko;

/**
 * Rust object allocated on the heap.
 */
public abstract class Heaped implements AutoCloseable {

  protected final int handle;
  private boolean freed = false;
  private boolean consumed = false;

  protected Heaped(final int handle) {
    this.handle = handle;
  }

  @Override
  public void close() {
    if (!freed) {
      drop();
      freed = true;
      consumed = true;
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
   * @throws AssertionError If the object is not alive.
   */
  protected void assertAlive() {
    assert !freed : "Attempt of use after free.";
    assert !consumed : "Attempt to manipulate the object after it's consumed!";
  }

  /**
   * Marks the object as consumed.
   */
  protected void consume() {
    assertAlive();
    consumed = true;
  }
}
