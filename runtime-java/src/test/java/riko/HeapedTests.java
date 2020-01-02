package riko;

import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.Test;

class HeapedTests {

  static class DummyHeaped extends Heaped {

    DummyHeaped() {
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
    final DummyHeaped obj = new DummyHeaped();
    Assertions.assertThrows(AssertionError.class, () -> {
      obj.close();
      obj.run();
    });
  }
}