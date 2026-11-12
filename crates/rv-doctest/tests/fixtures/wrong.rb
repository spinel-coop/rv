# Has a wrong example.
class Wrong
  # Wrong example.
  #
  # ```ruby
  # require "minitest/autorun"
  # def add(a, b) = a + b
  # assert_equal 99, add(1, 2)
  # ```
  def add(a, b)
    a + b
  end
end
