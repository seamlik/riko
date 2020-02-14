package riko;

import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.Test;

class ObjectTests {

  static class DummyObject extends Object {

    DummyObject() {
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
    final DummyObject obj = new DummyObject();
    Assertions.assertThrows(AssertionError.class, () -> {
      obj.close();
      obj.run();
    });
  }
}