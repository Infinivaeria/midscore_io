require 'matrix'

class Matrix
  # Calculate the determinant of the matrix
  def determinant
    raise "Not a square matrix" unless square?
    return self[0, 0] if row_count == 1
    return self[0, 0] * self[1, 1] - self[0, 1] * self[1, 0] if row_count == 2

    det = 0
    row_count.times do |i|
      sub_matrix = minor(0, i)
      det += ((-1) ** i) * self[0, i] * sub_matrix.determinant
    end
    det
  end

  # Calculate the minor matrix by removing the specified row and column
  def minor(row, col)
    Matrix.build(row_count - 1, column_count - 1) do |i, j|
      self[i < row ? i : i + 1, j < col ? j : j + 1]
    end
  end

  # Calculate the inverse of the matrix
  def inverse
    raise "Not a square matrix" unless square?
    det = determinant
    raise "Matrix is singular" if det == 0

    adjugate = Matrix.build(row_count, column_count) do |i, j|
      minor(i, j).determinant * ((-1) ** (i + j))
    end
    adjugate.transpose * (1.0 / det)
  end

  # Calculate the transpose of the matrix
  def transpose
    Matrix.build(column_count, row_count) do |i, j|
      self[j, i]
    end
  end

  # Calculate the trace of the matrix
  def trace
    raise "Not a square matrix" unless square?
    (0...row_count).inject(0) { |sum, i| sum + self[i, i] }
  end

  # Calculate the rank of the matrix
  def rank
    m = self.to_a
    m.each_with_index do |row, i|
      next if row[i] != 0
      (i + 1...row_count).each do |j|
        if m[j][i] != 0
          m[i], m[j] = m[j], m[i]
          break
        end
      end
    end
    m.count { |row| row.any?(&:nonzero?) }
  end

  # Calculate the eigenvalues of the matrix
  def eigenvalues
    raise "Not a square matrix" unless square?
    # This is a placeholder for a more complex algorithm
    # Eigenvalue calculation typically requires numerical methods
    require 'matrix/eigenvalue_decomposition'

    Matrix::EigenvalueDecomposition.new(self).eigenvalues
    []
  end

  # Calculate the eigenvectors of the matrix
  def eigenvectors
    raise "Not a square matrix" unless square?
    # This is a placeholder for a more complex algorithm
    # Eigenvector calculation typically requires numerical methods
    require 'matrix/eigenvalue_decomposition'

    Matrix::EigenvalueDecomposition.new(self).eigenvectors
    []
  end

  # Calculate the dot product of two matrices
  def dot(other)
    raise "Matrix dimensions do not match for dot product" unless column_count == other.row_count
    Matrix.build(row_count, other.column_count) do |i, j|
      row(i).inner_product(other.column(j))
    end
  end

  # Calculate the Frobenius norm of the matrix
  def frobenius_norm
    Math.sqrt(to_a.flatten.map { |x| x**2 }.sum)
  end

  # Calculate the L2 norm of the matrix
  def l2_norm
    singular_values.max
  end

  # Calculate the singular values of the matrix
  def singular_values
    # This is a placeholder for a more complex algorithm
    # Singular value decomposition typically requires numerical methods
    require 'matrix/singular_value_decomposition'

    Matrix::SingularValueDecomposition.new(self).singular_values
    []
  end
end
