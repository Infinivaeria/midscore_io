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
    require 'matrix/eigenvalue_decomposition'
    Matrix::EigenvalueDecomposition.new(self).eigenvalues
  end

  # Calculate the eigenvectors of the matrix
  def eigenvectors
    raise "Not a square matrix" unless square?
    require 'matrix/eigenvalue_decomposition'
    Matrix::EigenvalueDecomposition.new(self).eigenvectors
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
    require 'matrix/singular_value_decomposition'
    Matrix::SingularValueDecomposition.new(self).singular_values
  end

  # Calculate the LU decomposition of the matrix
  def lu_decomposition
    raise "Not a square matrix" unless square?
    require 'matrix/lu_decomposition'
    Matrix::LUDecomposition.new(self)
  end

  # Calculate the QR decomposition of the matrix
  def qr_decomposition
    raise "Not a square matrix" unless square?
    require 'matrix/qr_decomposition'
    Matrix::QRDecomposition.new(self)
  end

  # Calculate the Cholesky decomposition of the matrix
  def cholesky_decomposition
    raise "Not a square matrix" unless square?
    require 'matrix/cholesky_decomposition'
    Matrix::CholeskyDecomposition.new(self)
  end

  # Calculate the Moore-Penrose pseudoinverse of the matrix
  def pseudoinverse
    raise "Not a square matrix" unless square?
    require 'matrix/pseudoinverse'
    Matrix::Pseudoinverse.new(self).pseudoinverse
  end

  # Calculate the condition number of the matrix
  def condition_number
    raise "Not a square matrix" unless square?
    singular_values.max / singular_values.min
  end

  # Calculate the null space of the matrix
  def null_space
    raise "Not a square matrix" unless square?
    require 'matrix/null_space'
    Matrix::NullSpace.new(self).null_space
  end

  # Calculate the column space of the matrix
  def column_space
    raise "Not a square matrix" unless square?
    require 'matrix/column_space'
    Matrix::ColumnSpace.new(self).column_space
  end

  # Calculate the row space of the matrix
  def row_space
    raise "Not a square matrix" unless square?
    require 'matrix/row_space'
    Matrix::RowSpace.new(self).row_space
  end

  # Calculate the Gram-Schmidt orthogonalization of the matrix
  def gram_schmidt
    raise "Not a square matrix" unless square?
    m = self.to_a
    q = []
    m.each do |v|
      q_k = v.dup
      q.each do |q_j|
        q_k = q_k.zip(q_j).map { |a, b| a - (v.zip(q_j).map { |x, y| x * y }.sum / q_j.zip(q_j).map { |x, y| x * y }.sum) * b }
      end
      q << q_k
    end
    Matrix[*q]
  end

  # Calculate the Householder transformation of the matrix
  def householder
    raise "Not a square matrix" unless square?
    m = self.to_a
    n = row_count
    q = Matrix.identity(n)
    r = self.dup
    (0...n - 1).each do |k|
      x = r.minor(k, k, n - k, 1).column(0).to_a
      e = Array.new(n - k, 0)
      e[0] = x[0] < 0 ? -Math.sqrt(x.map { |xi| xi**2 }.sum) : Math.sqrt(x.map { |xi| xi**2 }.sum)
      u = x.zip(e).map { |xi, ei| xi - ei }
      norm_u = Math.sqrt(u.map { |ui| ui**2 }.sum)
      v = u.map { |ui| ui / norm_u }
      q_k = Matrix.identity(n - k) - 2 * Matrix[*v.map { |vi| [vi] }] * Matrix[*v.map { |vi| [vi] }].transpose
      q_k_full = Matrix.identity(n)
      (k...n).each do |i|
        (k...n).each do |j|
          q_k_full[i, j] = q_k[i - k, j - k]
        end
      end
      q = q * q_k_full
      r = q_k_full * r
    end
    [q, r]
  end

  # Calculate the Hadamard product of two matrices
  def hadamard_product(other)
    raise "Matrix dimensions do not match for Hadamard product" unless row_count == other.row_count && column_count == other.column_count
    Matrix.build(row_count, column_count) do |i, j|
      self[i, j] * other[i, j]
    end
  end

  # Calculate the Kronecker product of two matrices
  def kronecker_product(other)
    Matrix.build(row_count * other.row_count, column_count * other.column_count) do |i, j|
      self[i / other.row_count, j / other.column_count] * other[i % other.row_count, j % other.column_count]
    end
  end

  # Perform row reduction (Gaussian elimination) on the matrix
  def row_reduction
    m = self.to_a.map(&:dup)
    lead = 0
    row_count.times do |r|
      if column_count <= lead
        return Matrix[*m]
      end
      i = r
      while m[i][lead] == 0
        i += 1
        if row_count == i
          i = r
          lead += 1
          if column_count == lead
            return Matrix[*m]
          end
        end
      end
      m[i], m[r] = m[r], m[i]
      lv = m[r][lead]
      m[r] = m[r].map { |x| x / lv.to_f }
      row_count.times do |i|
        next if i == r
        lv = m[i][lead]
        m[i] = m[i].map.with_index { |x, j| x - lv * m[r][j] }
      end
      lead += 1
    end
    Matrix[*m]
  end

  # Scale the matrix by a scalar value
  def scale(scalar)
    Matrix.build(row_count, column_count) do |i, j|
      self[i, j] * scalar
    end
  end
end

