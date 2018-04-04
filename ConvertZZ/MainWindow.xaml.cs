using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;

namespace ConvertZZ
{
    /// <summary>
    /// MainWindow.xaml 的互動邏輯
    /// </summary>
    public partial class MainWindow : Window
    {
        public MainWindow()
        {
            InitializeComponent();
            Console.WriteLine(ClipBoardHelper.GetClipBoard());
        }
        Point pointNow = new Point();
        bool leftDown = false;
        private void Window_MouseDown(object sender, MouseButtonEventArgs e)
        {
            if (e.LeftButton == MouseButtonState.Pressed)
            {
                pointNow = new Point(Left, Top);
                leftDown = true;
                this.DragMove();
            }
        }
        private void Setting_Click(object sender, RoutedEventArgs e)
        {
            
        }
    
        private void Exit_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
        }

        private void Window_MouseUp(object sender, MouseButtonEventArgs e)
        {
            if (Left == pointNow.X && Top == pointNow.Y && leftDown)
            {
                leftDown = false;
                if (Keyboard.IsKeyDown(Key.LeftCtrl) || Keyboard.IsKeyDown(Key.RightCtrl))
                    MessageBox.Show("ctrl");
                else
                    MessageBox.Show("");
            }
        }
    }

}
