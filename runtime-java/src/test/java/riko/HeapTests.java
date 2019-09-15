package riko;

import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.Test;

class HeapTests {

  static class DummyHeap extends Heap {

    DummyHeap() {
      super(0);
    }

    @Override
    protected void drop() {
      // Nothing
    }

    void run() {
      assertAlive();
      // Nothing
    }
  }

  @Test
  void throwsWhenUseAfterFree() {
    final DummyHeap obj = new DummyHeap();
    Assertions.assertThrows(AssertionError.class, () -> {
      obj.close();
      obj.run();
    });
  }
}